use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use xtask_support::capture;

use crate::consumers::Lane;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Queued,
    InProgress,
    Completed,
    Waiting,
    Pending,
    Requested,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Conclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
    Stale,
    StartupFailure,
}

#[derive(Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Deserialize)]
struct Run {
    id: u64,
    run_number: u64,
    run_attempt: u64,
    path: String,
    event: String,
    status: Status,
    head_sha: String,
    repository: Repository,
}

#[derive(Deserialize)]
struct Runs {
    workflow_runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Step {
    name: String,
    status: Status,
    conclusion: Option<Conclusion>,
}

#[derive(Deserialize)]
struct Job {
    id: u64,
    run_id: u64,
    run_attempt: u64,
    name: String,
    head_sha: String,
    status: Status,
    conclusion: Option<Conclusion>,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<Job>,
}

struct Proof {
    run: u64,
    attempt: u64,
    job: u64,
    revision: String,
}

enum Observation {
    Pending(&'static str),
    Passed(Proof),
}

fn repository_valid(repo: &str) -> bool {
    let parts: Vec<_> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|part| {
            !matches!(*part, "" | "." | "..")
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}

fn observe(
    repo: &str,
    head: &str,
    event: &str,
    git_ref: &str,
    lane: Lane,
    mut gh: impl FnMut(&[&str]) -> Result<Vec<u8>>,
) -> Result<Observation> {
    ensure!(
        repository_valid(repo) && super::super::sha(head),
        "invalid API host repository or head"
    );
    ensure!(
        matches!(event, "push" | "pull_request"),
        "unsupported API host event"
    );
    let lane_name = super::lane_name(lane)?;
    let endpoint = format!(
        "repos/{repo}/actions/workflows/consumers.yml/runs?head_sha={head}&event={event}&per_page=100"
    );
    let pages: Vec<Runs> =
        serde_json::from_slice(&gh(&["api", "--paginate", "--slurp", &endpoint])?)?;
    let mut candidates: Vec<_> = pages
        .into_iter()
        .flat_map(|page| page.workflow_runs)
        .filter(|run| {
            run.path == ".github/workflows/consumers.yml"
                && run.head_sha == head
                && run.event == event
                && run.repository.full_name == repo
        })
        .collect();
    candidates.sort_by_key(|run| std::cmp::Reverse(run.run_number));
    let mut latest = None;
    for run in candidates {
        ensure!(
            run.run_attempt > 0 && run.run_number > 0,
            "API host workflow run number or attempt is missing"
        );
        let endpoint = format!(
            "repos/{repo}/actions/runs/{}/attempts/{}",
            run.id, run.run_attempt
        );
        let source = gh(&["api", &endpoint])?;
        let execution: Execution = serde_json::from_slice(&source)?;
        ensure!(
            !execution.referenced_workflows.is_empty(),
            "API host execution references are unavailable"
        );
        if !execution
            .referenced_workflows
            .iter()
            .any(|reference| reference.git_ref == git_ref)
        {
            continue;
        }
        let revision = execution_revision(&source, repo, git_ref)?;
        // A newer failed or pending run for this ref must never inherit an older success.
        latest = Some((run, revision));
        break;
    }
    let Some((run, revision)) = latest else {
        return Ok(Observation::Pending(
            "API host workflow is unavailable for this ref",
        ));
    };
    let endpoint = format!(
        "repos/{repo}/actions/runs/{}/attempts/{}/jobs?per_page=100",
        run.id, run.run_attempt
    );
    let pages: Vec<Jobs> =
        serde_json::from_slice(&gh(&["api", "--paginate", "--slurp", &endpoint])?)?;
    let name = format!("API hosts / Independent API hosts ({lane_name})");
    let matches: Vec<_> = pages
        .into_iter()
        .flat_map(|page| page.jobs)
        .filter(|job| job.name == name)
        .collect();
    let job = match matches.as_slice() {
        [job] => job,
        [] if run.status != Status::Completed => {
            return Ok(Observation::Pending("API host job is unavailable"));
        }
        _ => anyhow::bail!(
            "expected one {name} result in API host run {} attempt {}",
            run.id,
            run.run_attempt
        ),
    };
    ensure!(
        job.head_sha == head && job.run_id == run.id && job.run_attempt == run.run_attempt,
        "API host result belongs to a different head, run or attempt"
    );
    if job.status != Status::Completed {
        ensure!(
            job.conclusion.is_none(),
            "unfinished API host job has a conclusion"
        );
        return Ok(Observation::Pending("API host job is still running"));
    }
    ensure!(
        job.conclusion == Some(Conclusion::Success),
        "{name} failed to qualify {head}: {:?} in run {} attempt {}",
        job.conclusion,
        run.id,
        run.run_attempt
    );
    let step_name = if lane == Lane::Native {
        "Run isolated native profiles and shared negative controls"
    } else {
        "Run isolated MSRV profiles and shared negative controls"
    };
    let steps: Vec<_> = job
        .steps
        .iter()
        .filter(|step| step.name == step_name)
        .collect();
    ensure!(
        matches!(steps.as_slice(), [step] if step.status == Status::Completed && step.conclusion == Some(Conclusion::Success)),
        "{name} did not successfully execute its complete registered consumer suite"
    );
    Ok(Observation::Passed(Proof {
        run: run.id,
        attempt: run.run_attempt,
        job: job.id,
        revision,
    }))
}

// CI deadlines must follow elapsed host time, independent of application clocks.
#[allow(clippy::disallowed_methods)]
fn wait_for_result(
    timeout: Duration,
    mut observe: impl FnMut() -> Result<Observation>,
    mut sleep: impl FnMut(Duration),
) -> Result<Proof> {
    let started = Instant::now();
    loop {
        match observe()? {
            Observation::Passed(proof) => return Ok(proof),
            Observation::Pending(reason) => {
                ensure!(
                    started.elapsed() < timeout,
                    "timed out waiting for delegated consumer qualification: {reason}"
                );
                eprintln!("Waiting for delegated qualification: {reason}");
                sleep(Duration::from_secs(15).min(timeout.saturating_sub(started.elapsed())));
            }
        }
    }
}

#[derive(Deserialize)]
struct WorkflowReference {
    path: String,
    #[serde(rename = "ref")]
    git_ref: String,
    sha: String,
}

#[derive(Deserialize)]
struct Execution {
    referenced_workflows: Vec<WorkflowReference>,
}

fn execution_revision(source: &[u8], repo: &str, git_ref: &str) -> Result<String> {
    let execution: Execution = serde_json::from_slice(source)?;
    let prefix = format!("{repo}/.github/workflows/consumer-hosts.yml@");
    let matches: Vec<_> = execution
        .referenced_workflows
        .into_iter()
        .filter(|reference| {
            reference.git_ref == git_ref && reference.path == format!("{prefix}{}", reference.sha)
        })
        .collect();
    ensure!(
        matches.len() == 1 && super::super::sha(&matches[0].sha),
        "API host execution revision is unavailable or ambiguous"
    );
    Ok(matches[0].sha.clone())
}

fn execution_sources(
    repo: &str,
    revision: &str,
    lane: Lane,
    toolchain: &str,
    mut gh: impl FnMut(&[&str]) -> Result<Vec<u8>>,
) -> Result<()> {
    let mut read = |path: &str| -> Result<String> {
        let endpoint = format!("repos/{repo}/contents/{path}?ref={revision}");
        Ok(String::from_utf8(gh(&[
            "api",
            "-H",
            "Accept: application/vnd.github.raw+json",
            &endpoint,
        ])?)?)
    };
    super::verify_wrapper(&read(".github/workflows/consumers.yml")?)?;
    super::verify_execution(
        &read(".github/workflows/consumer-hosts.yml")?,
        &read("rust-toolchain.toml")?,
        lane,
        toolchain,
    )
}

pub fn verify(root: &Path, lane: Lane, head: &str, toolchain: &str) -> Result<()> {
    let repo = std::env::var("GITHUB_REPOSITORY")
        .context("delegated result requires GITHUB_REPOSITORY")?;
    let event = std::env::var("GITHUB_EVENT_NAME")
        .context("delegated result requires GITHUB_EVENT_NAME")?;
    let gh = |args: &[&str]| -> Result<Vec<u8>> {
        Ok(capture(
            Command::new("timeout")
                .args(["45", "gh"])
                .args(args)
                .current_dir(root),
        )?
        .stdout)
    };
    let git_ref = std::env::var("GITHUB_REF").context("delegated result requires GITHUB_REF")?;
    let proof = wait_for_result(
        Duration::from_secs(900),
        || observe(&repo, head, &event, &git_ref, lane, gh),
        std::thread::sleep,
    )?;
    let revision = &proof.revision;
    execution_sources(&repo, revision, lane, toolchain, gh)?;
    println!(
        "Verified delegated {lane:?} checkout consumers at {head}, workflow revision {revision}, attempt {}: https://github.com/{repo}/actions/runs/{}/job/{}",
        proof.attempt, proof.run, proof.job
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn fixture() -> (Value, Value) {
        (
            json!([{"workflow_runs":[{"id":42,"run_number":4,"run_attempt":2,"path":".github/workflows/consumers.yml","event":"pull_request","status":"completed","head_sha":HEAD,"repository":{"full_name":"example/project"}}]}]),
            json!([{"jobs":[{"id":99,"run_id":42,"run_attempt":2,"name":"API hosts / Independent API hosts (native)","head_sha":HEAD,"status":"completed","conclusion":"success","steps":[{"name":"Run isolated native profiles and shared negative controls","status":"completed","conclusion":"success"}]}]}]),
        )
    }

    fn execution_fixture() -> Vec<u8> {
        serde_json::to_vec(&json!({"referenced_workflows":[{"path":format!("example/project/.github/workflows/consumer-hosts.yml@{HEAD}"),"ref":"refs/pull/7/merge","sha":HEAD}]})).unwrap()
    }

    fn exercise(runs: Value, jobs: Value) -> Result<Observation> {
        observe(
            "example/project",
            HEAD,
            "pull_request",
            "refs/pull/7/merge",
            Lane::Native,
            |args| {
                if args.len() == 2 {
                    return Ok(execution_fixture());
                }
                match args[3] {
                    value if value.contains("/workflows/") => Ok(serde_json::to_vec(&runs)?),
                    "repos/example/project/actions/runs/42/attempts/2/jobs?per_page=100" => {
                        Ok(serde_json::to_vec(&jobs)?)
                    }
                    value => anyhow::bail!("unexpected endpoint {value}"),
                }
            },
        )
    }

    #[test]
    fn execution_reference_is_bound_to_the_event_ref_and_repository() {
        let fixture = json!({"referenced_workflows":[{"path":format!("example/project/.github/workflows/consumer-hosts.yml@{HEAD}"),"ref":"refs/pull/7/merge","sha":HEAD}]});
        let bytes = serde_json::to_vec(&fixture).unwrap();
        assert_eq!(
            execution_revision(&bytes, "example/project", "refs/pull/7/merge").unwrap(),
            HEAD
        );
        assert!(execution_revision(&bytes, "example/project", "refs/pull/8/merge").is_err());
        assert!(execution_revision(&bytes, "other/project", "refs/pull/7/merge").is_err());
        assert!(
            execution_revision(
                br#"{"referenced_workflows":[]}"#,
                "example/project",
                "refs/pull/7/merge"
            )
            .is_err()
        );
    }

    #[test]
    fn actual_execution_configuration_must_preserve_the_requested_compiler() {
        let host = include_str!("../../../../../.github/workflows/consumer-hosts.yml");
        let wrapper = include_str!("../../../../../.github/workflows/consumers.yml");
        let check = |host: &str| {
            execution_sources("example/project", HEAD, Lane::Native, "1.100.0", |args| {
                let endpoint = args[3];
                assert!(endpoint.ends_with(&format!("?ref={HEAD}")));
                Ok(if endpoint.contains("consumer-hosts.yml") {
                    host
                } else if endpoint.contains("consumers.yml") {
                    wrapper
                } else {
                    "[toolchain]\nchannel = \"1.100.0\""
                }
                .as_bytes()
                .to_vec())
            })
        };
        check(host).unwrap();
        assert!(check(&host.replace("--toolchain 1.100.0", "--toolchain 1.95.0")).is_err());
    }

    #[test]
    fn exact_head_and_current_attempt_success_is_required() {
        let (runs, jobs) = fixture();
        assert!(matches!(
            exercise(runs.clone(), jobs.clone()).unwrap(),
            Observation::Passed(Proof {
                run: 42,
                attempt: 2,
                job: 99,
                ..
            })
        ));
        for (path, value) in [
            ("/0/jobs/0/head_sha", json!("b".repeat(40))),
            ("/0/jobs/0/run_id", json!(1)),
            ("/0/jobs/0/run_attempt", json!(1)),
        ] {
            let mut jobs = jobs.clone();
            *jobs.pointer_mut(path).unwrap() = value;
            assert!(exercise(runs.clone(), jobs).is_err());
        }
    }

    #[test]
    fn failed_cancelled_skipped_or_unexecuted_consumers_fail_the_gate() {
        let (runs, jobs) = fixture();
        for (path, value) in [
            ("/0/jobs/0/conclusion", json!("failure")),
            ("/0/jobs/0/conclusion", json!("cancelled")),
            ("/0/jobs/0/conclusion", json!("skipped")),
            ("/0/jobs/0/steps/0/conclusion", json!("skipped")),
            ("/0/jobs/0/steps", json!([])),
        ] {
            let mut jobs = jobs.clone();
            *jobs.pointer_mut(path).unwrap() = value;
            assert!(exercise(runs.clone(), jobs).is_err());
        }
    }

    #[test]
    fn unrelated_workflow_head_event_or_repository_cannot_supply_evidence() {
        let (runs, jobs) = fixture();
        for (field, value) in [
            ("path", json!(".github/workflows/other.yml")),
            ("head_sha", json!("b".repeat(40))),
            ("event", json!("workflow_dispatch")),
            ("repository", json!({"full_name":"other/project"})),
        ] {
            let mut runs = runs.clone();
            runs[0]["workflow_runs"][0][field] = value;
            assert!(matches!(
                exercise(runs, jobs.clone()).unwrap(),
                Observation::Pending(_)
            ));
        }
    }

    #[test]
    fn newer_run_for_another_pull_request_cannot_block_this_ref() {
        let (mut runs, jobs) = fixture();
        let mut other = runs[0]["workflow_runs"][0].clone();
        other["id"] = json!(43);
        other["run_number"] = json!(5);
        runs[0]["workflow_runs"].as_array_mut().unwrap().push(other);
        let result = observe(
            "example/project",
            HEAD,
            "pull_request",
            "refs/pull/7/merge",
            Lane::Native,
            |args| {
                let endpoint = *args.last().unwrap();
                if endpoint.contains("/workflows/") {
                    return Ok(serde_json::to_vec(&runs)?);
                }
                if endpoint.ends_with("/attempts/2") {
                    let git_ref = if endpoint.contains("/runs/43/") {
                        "refs/pull/8/merge"
                    } else {
                        "refs/pull/7/merge"
                    };
                    return Ok(serde_json::to_vec(
                        &json!({"referenced_workflows":[{"path":format!("example/project/.github/workflows/consumer-hosts.yml@{HEAD}"),"ref":git_ref,"sha":HEAD}]}),
                    )?);
                }
                ensure!(
                    endpoint.contains("/runs/42/"),
                    "selected the other pull request"
                );
                Ok(serde_json::to_vec(&jobs)?)
            },
        );
        assert!(matches!(
            result.unwrap(),
            Observation::Passed(Proof { run: 42, .. })
        ));
    }

    #[test]
    fn newest_run_is_selected_before_any_success_and_attempt_is_exact() {
        let (mut runs, mut jobs) = fixture();
        let mut old = runs[0]["workflow_runs"][0].clone();
        old["id"] = json!(43);
        old["run_number"] = json!(3);
        old["run_attempt"] = json!(1);
        runs.as_array_mut()
            .unwrap()
            .insert(0, json!({"workflow_runs":[old]}));
        let mut old_jobs = jobs.clone();
        old_jobs[0]["jobs"][0]["run_id"] = json!(43);
        old_jobs[0]["jobs"][0]["run_attempt"] = json!(1);
        jobs[0]["jobs"][0]["conclusion"] = json!("failure");
        let result = observe(
            "example/project",
            HEAD,
            "pull_request",
            "refs/pull/7/merge",
            Lane::Native,
            |args| {
                if args.len() == 2 {
                    return Ok(execution_fixture());
                }
                if args[3].contains("/workflows/") {
                    return Ok(serde_json::to_vec(&runs)?);
                }
                let response = if args[3].contains("/runs/43/") {
                    &old_jobs
                } else {
                    &jobs
                };
                Ok(serde_json::to_vec(response)?)
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn missing_or_pending_results_cannot_satisfy_the_gate() {
        let (mut runs, mut jobs) = fixture();
        assert!(exercise(runs.clone(), json!([{"jobs":[]}])).is_err());
        runs[0]["workflow_runs"][0]["status"] = json!("queued");
        assert!(matches!(
            exercise(runs.clone(), json!([{"jobs":[]}])).unwrap(),
            Observation::Pending(_)
        ));
        jobs[0]["jobs"][0]["status"] = json!("in_progress");
        jobs[0]["jobs"][0]["conclusion"] = Value::Null;
        assert!(matches!(
            exercise(runs, jobs).unwrap(),
            Observation::Pending(_)
        ));
        assert!(
            wait_for_result(
                Duration::ZERO,
                || Ok(Observation::Pending("missing")),
                |_| panic!("deadline must fail without sleeping")
            )
            .is_err()
        );
    }

    #[test]
    fn ambiguous_jobs_and_api_failures_are_errors() {
        let (runs, mut jobs) = fixture();
        let duplicate = jobs[0]["jobs"][0].clone();
        jobs[0]["jobs"].as_array_mut().unwrap().push(duplicate);
        assert!(exercise(runs, jobs).is_err());
        assert!(
            observe(
                "example/project",
                HEAD,
                "pull_request",
                "refs/pull/7/merge",
                Lane::Native,
                |_| anyhow::bail!("API unavailable")
            )
            .is_err()
        );
    }
}
