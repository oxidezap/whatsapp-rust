use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Repository {
    full_name: String,
}
#[derive(Deserialize)]
struct Run {
    id: u64,
    run_attempt: u64,
    path: String,
    event: String,
    status: String,
    head_branch: String,
    head_sha: String,
    repository: Repository,
    head_repository: Repository,
}
#[derive(Deserialize)]
struct Runs {
    workflow_runs: Vec<Run>,
}
#[derive(Deserialize)]
struct Step {
    name: String,
    number: u64,
    conclusion: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
}
#[derive(Deserialize)]
struct Job {
    name: String,
    head_sha: String,
    status: String,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Jobs {
    jobs: Vec<Job>,
}
#[derive(Deserialize)]
struct ArtifactRun {
    id: u64,
    head_branch: String,
    head_sha: String,
}
#[derive(Deserialize)]
struct Artifact {
    name: String,
    expired: bool,
    created_at: String,
    workflow_run: ArtifactRun,
}
#[derive(Deserialize)]
struct Artifacts {
    artifacts: Vec<Artifact>,
}

pub fn download(
    head: &Path,
    destination: &Path,
    repo: &str,
    sha: &str,
    mut gh: impl FnMut(&[&str]) -> Result<Vec<u8>>,
) -> Result<()> {
    ensure!(
        !destination.exists(),
        "baseline directory already exists; refusing stale artifacts"
    );
    ensure!(
        sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "BASE_SHA must be the full PR base commit"
    );
    ensure!(
        repo.split('/').count() == 2
            && repo.split('/').all(|part| !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))),
        "invalid base repository"
    );
    let endpoint = format!(
        "repos/{repo}/actions/workflows/binary-size.yml/runs?branch=main&head_sha={sha}&status=completed&per_page=100"
    );
    let pages: Vec<Runs> =
        serde_json::from_slice(&gh(&["api", "--paginate", "--slurp", &endpoint])?)?;
    for run in pages.into_iter().flat_map(|page| page.workflow_runs) {
        if run.path != ".github/workflows/binary-size.yml"
            || !matches!(run.event.as_str(), "push" | "workflow_dispatch")
            || run.status != "completed"
            || run.head_branch != "main"
            || run.head_sha != sha
            || run.repository.full_name != repo
            || run.head_repository.full_name != repo
        {
            continue;
        }
        let endpoint = format!(
            "repos/{repo}/actions/runs/{}/attempts/{}/jobs?per_page=100",
            run.id, run.run_attempt
        );
        let pages: Vec<Jobs> =
            serde_json::from_slice(&gh(&["api", "--paginate", "--slurp", &endpoint])?)?;
        let upload = pages.iter().flat_map(|page| &page.jobs).find_map(|job| {
            if job.name != "Binary Size (push)" || job.head_sha != sha || job.status != "completed"
            {
                return None;
            }
            let measure = job
                .steps
                .iter()
                .find(|step| step.name == "Measure binary size")?;
            let upload = job
                .steps
                .iter()
                .find(|step| step.name == "Upload size artifacts")?;
            (measure.conclusion.as_deref() == Some("success")
                && upload.conclusion.as_deref() == Some("success")
                && measure.number < upload.number)
                .then_some(upload)
        });
        let Some(upload) = upload else {
            eprintln!(
                "Ignoring size run {} without successful measurement and upload",
                run.id
            );
            continue;
        };
        let endpoint = format!(
            "repos/{repo}/actions/runs/{}/artifacts?per_page=100",
            run.id
        );
        let pages: Vec<Artifacts> =
            serde_json::from_slice(&gh(&["api", "--paginate", "--slurp", &endpoint])?)?;
        let artifacts: Vec<_> = pages
            .iter()
            .flat_map(|page| &page.artifacts)
            .filter(|artifact| artifact.name == "size-metrics")
            .collect();
        let [artifact] = artifacts.as_slice() else {
            eprintln!(
                "Ignoring size run {} without one unambiguous size-metrics artifact",
                run.id
            );
            continue;
        };
        // Reruns share a run ID. An old attempt's artifact must not borrow the new attempt's status.
        if artifact.expired
            || artifact.workflow_run.id != run.id
            || artifact.workflow_run.head_branch != "main"
            || artifact.workflow_run.head_sha != sha
            || !upload
                .started_at
                .as_ref()
                .is_some_and(|start| start <= &artifact.created_at)
            || !upload
                .completed_at
                .as_ref()
                .is_some_and(|end| &artifact.created_at <= end)
        {
            eprintln!(
                "Ignoring size run {} with expired or inconsistent artifact provenance",
                run.id
            );
            continue;
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent)?;
        let temporary = tempfile::tempdir_in(parent)?;
        gh(&[
            "run",
            "download",
            &run.id.to_string(),
            "--repo",
            repo,
            "--name",
            "size-metrics",
            "--dir",
            temporary
                .path()
                .to_str()
                .context("baseline path encoding")?,
        ])?;
        if let Err(error) = super::size::validate_baseline(head, temporary.path(), Some(sha)) {
            if error.is::<super::size::CompilerMismatch>() {
                eprintln!(
                    "Ignoring size run {} measured with a different compiler",
                    run.id
                );
                continue;
            }
            return Err(error)
                .with_context(|| format!("invalid size baseline from run {}", run.id));
        }
        std::fs::rename(temporary.path(), destination)?;
        println!("Validated size baseline {sha} from main run {}", run.id);
        return Ok(());
    }
    anyhow::bail!(
        "No validated size-metrics artifact for PR base {sha} on {repo} main; run Binary Size on that base commit before retrying. The size gate cannot be skipped."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use xtask_support::write_json;

    const SHA: &str = "47e1b5b41b63c23b59372828901ea945c8149565";
    const OLD: &str = "2b9a8d799ec2da008cc47fbd1c61d8995d652238";
    const REPO: &str = "example/project";

    struct Fixture {
        runs: Value,
        jobs: Value,
        artifacts: Value,
        meta: Value,
        metrics: Value,
        fail_download: bool,
    }
    fn fixture() -> Fixture {
        let run = json!({
            "id":42,"run_attempt":2,"path":".github/workflows/binary-size.yml",
            "event":"push","status":"completed","conclusion":"failure",
            "head_branch":"main","head_sha":SHA,
            "repository":{"full_name":REPO},"head_repository":{"full_name":REPO}
        });
        let mut old = run.clone();
        old["id"] = json!(41);
        old["head_sha"] = json!(OLD);
        old["conclusion"] = json!("success");
        Fixture {
            runs: json!([{"workflow_runs":[old]},{"workflow_runs":[run]}]),
            jobs: json!([{"jobs":[{
                "name":"Binary Size (push)","head_sha":SHA,"status":"completed",
                "steps":[
                    {"name":"Measure binary size","number":9,"conclusion":"success"},
                    {"name":"Upload size artifacts","number":10,"conclusion":"success",
                        "started_at":"2026-09-08T00:33:37Z","completed_at":"2026-09-08T00:33:39Z"},
                    {"name":"Store series on gh-pages","number":11,"conclusion":"failure"}
                ]
            }]}]),
            artifacts: json!([{"artifacts":[{
                "name":"size-metrics","expired":false,"created_at":"2026-09-08T00:33:38Z",
                "workflow_run":{"id":42,"head_branch":"main","head_sha":SHA}
            }]}]),
            meta: json!({"commit":SHA,"rustc":"rustc 1.98.0-nightly (01dfd7924 2026-06-15)"}),
            metrics: json!([
                {"name":"bin size (stripped)","unit":"bytes","value":11129496},
                {"name":"bin .text","unit":"bytes","value":8968502}
            ]),
            fail_download: false,
        }
    }
    fn exercise(fixture: Fixture) -> (Result<()>, usize) {
        let root = tempfile::tempdir().unwrap();
        write_json(
            &root.path().join("size-out/size-metrics.json"),
            &self::fixture().metrics,
        )
        .unwrap();
        write_json(
            &root.path().join("size-out/size-meta.json"),
            &self::fixture().meta,
        )
        .unwrap();
        let mut downloads = 0;
        let result = download(
            &root.path().join("size-out"),
            &root.path().join("base-out"),
            REPO,
            SHA,
            |args| {
                if args[0] == "api" {
                    assert_eq!(&args[..3], &["api", "--paginate", "--slurp"]);
                    let value = match args[3] {
                        path if path
                            == format!(
                                "repos/{REPO}/actions/workflows/binary-size.yml/runs?branch=main&head_sha={SHA}&status=completed&per_page=100"
                            ) =>
                        {
                            fixture.runs.clone()
                        }
                        "repos/example/project/actions/runs/42/attempts/2/jobs?per_page=100"
                        | "repos/example/project/actions/runs/43/attempts/2/jobs?per_page=100" => {
                            fixture.jobs.clone()
                        }
                        "repos/example/project/actions/runs/42/artifacts?per_page=100" => {
                            fixture.artifacts.clone()
                        }
                        "repos/example/project/actions/runs/43/artifacts?per_page=100" => {
                            let mut artifacts = fixture.artifacts.clone();
                            artifacts[0]["artifacts"][0]["workflow_run"]["id"] = json!(43);
                            artifacts
                        }
                        path => panic!("unexpected GitHub query: {path}"),
                    };
                    Ok(serde_json::to_vec(&value)?)
                } else {
                    assert!(matches!(args[2], "42" | "43"));
                    assert_eq!(
                        &args[..8],
                        &[
                            "run",
                            "download",
                            args[2],
                            "--repo",
                            REPO,
                            "--name",
                            "size-metrics",
                            "--dir"
                        ]
                    );
                    downloads += 1;
                    ensure!(!fixture.fail_download, "GitHub artifact download failed");
                    let out = Path::new(args[8]);
                    let mut meta = fixture.meta.clone();
                    if args[2] == "43" {
                        meta["rustc"] = json!("different rustc");
                    }
                    write_json(&out.join("size-meta.json"), &meta)?;
                    write_json(&out.join("size-metrics.json"), &fixture.metrics)?;
                    Ok(Vec::new())
                }
            },
        );
        assert_eq!(root.path().join("base-out").exists(), result.is_ok());
        (result, downloads)
    }
    #[test]
    fn exact_base_survives_graph_failure_and_stale_successful_main() {
        let (result, downloads) = exercise(fixture());
        result.unwrap();
        assert_eq!(downloads, 1);
        let mut f = fixture();
        f.runs[1]["workflow_runs"][0]["event"] = json!("workflow_dispatch");
        exercise(f).0.unwrap();
    }
    #[test]
    fn rejects_failed_measurement_upload_and_untrusted_runs_before_download() {
        for (pointer, value) in [
            ("/0/jobs/0/steps/0/conclusion", json!("failure")),
            ("/0/jobs/0/steps/1/conclusion", json!("skipped")),
            ("/0/jobs/0/steps/1/number", json!(8)),
            ("/0/jobs/0/head_sha", json!(OLD)),
        ] {
            let mut f = fixture();
            *f.jobs.pointer_mut(pointer).unwrap() = value;
            let (result, downloads) = exercise(f);
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("gate cannot be skipped")
            );
            assert_eq!(downloads, 0);
        }
        for (field, value) in [
            ("event", json!("pull_request")),
            ("head_branch", json!("feature")),
            ("head_sha", json!(OLD)),
            ("path", json!(".github/workflows/other.yml")),
            ("head_repository", json!({"full_name":"fork/project"})),
            ("repository", json!({"full_name":"fork/project"})),
        ] {
            let mut f = fixture();
            f.runs[1]["workflow_runs"][0][field] = value;
            let (result, downloads) = exercise(f);
            assert!(result.is_err(), "{field}");
            assert_eq!(downloads, 0);
        }
    }
    #[test]
    fn rejects_expired_mismatched_and_previous_attempt_artifacts() {
        for (pointer, value) in [
            ("/0/artifacts/0/expired", json!(true)),
            ("/0/artifacts/0/workflow_run/head_sha", json!(OLD)),
            ("/0/artifacts/0/workflow_run/id", json!(41)),
            ("/0/artifacts/0/created_at", json!("2026-09-07T00:33:38Z")),
            ("/0/artifacts", json!([])),
        ] {
            let mut f = fixture();
            *f.artifacts.pointer_mut(pointer).unwrap() = value;
            let (result, downloads) = exercise(f);
            assert!(result.is_err(), "{pointer}");
            assert_eq!(downloads, 0);
        }
    }
    #[test]
    fn rejects_downloaded_commit_and_compiler_mismatches() {
        for (field, value) in [("commit", json!(OLD)), ("rustc", json!("different rustc"))] {
            let mut f = fixture();
            f.meta[field] = value;
            let (result, downloads) = exercise(f);
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(if field == "rustc" {
                        "gate cannot be skipped"
                    } else {
                        "invalid size baseline"
                    })
            );
            assert_eq!(downloads, 1);
        }
    }
    #[test]
    fn alternate_compiler_dispatch_does_not_hide_matching_main_measurement() {
        let mut f = fixture();
        let mut newer = f.runs[1]["workflow_runs"][0].clone();
        newer["id"] = json!(43);
        newer["event"] = json!("workflow_dispatch");
        f.runs[1]["workflow_runs"]
            .as_array_mut()
            .unwrap()
            .insert(0, newer);
        let (result, downloads) = exercise(f);
        result.unwrap();
        assert_eq!(downloads, 2);
    }
    #[test]
    fn unavailable_or_invalid_evidence_is_an_error_not_a_skipped_gate() {
        let mut f = fixture();
        f.fail_download = true;
        assert!(
            exercise(f)
                .0
                .unwrap_err()
                .to_string()
                .contains("download failed")
        );
        let mut f = fixture();
        f.runs = json!([{"workflow_runs":[]}]);
        assert!(
            exercise(f)
                .0
                .unwrap_err()
                .to_string()
                .contains("gate cannot be skipped")
        );
        let mut f = fixture();
        f.metrics = json!([]);
        assert!(exercise(f).0.is_err());
        let mut f = fixture();
        f.meta = json!({});
        assert!(exercise(f).0.is_err());
        let mut f = fixture();
        let duplicate = f.artifacts[0]["artifacts"][0].clone();
        f.artifacts[0]["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(exercise(f).0.is_err());
        let root = tempfile::tempdir().unwrap();
        let error = download(root.path(), &root.path().join("base"), REPO, SHA, |_| {
            anyhow::bail!("GitHub API unavailable")
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "GitHub API unavailable");
    }
}
