use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Sample {
    checkpoint: String,
    rust_live_bytes: u64,
    rust_peak_bytes: u64,
    rss_anon_kib: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Run {
    backend: String,
    mode: String,
    lanes: usize,
    messages: usize,
    text_bytes: usize,
    history_messages: usize,
    workload_seed: u64,
    idle_seconds: u64,
    samples: Vec<Sample>,
}

#[derive(Clone, Copy)]
struct Build<'a> {
    executable: &'a Path,
    revision: &'a str,
    role: &'static str,
}

#[derive(Serialize)]
struct SampleMetrics {
    live: u64,
    peak: u64,
    rss_anon_kib: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MedianBytes(i128);

impl Serialize for MedianBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut value = serializer.serialize_struct("MedianBytes", 2)?;
        value.serialize_field("numerator", &self.0)?;
        value.serialize_field("denominator", &2_u8)?;
        value.end()
    }
}

#[derive(Serialize)]
struct Comparison {
    checkpoint: String,
    base_retained_bytes: MedianBytes,
    head_retained_bytes: MedianBytes,
    delta_bytes: MedianBytes,
    regression: bool,
    base_samples: Vec<SampleMetrics>,
    head_samples: Vec<SampleMetrics>,
}

#[derive(Serialize)]
struct StreamReport {
    backend: &'static str,
    mode: &'static str,
    checkpoints: Vec<Comparison>,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    base_revision: String,
    head_revision: String,
    base_executable: PathBuf,
    head_executable: PathBuf,
    base_executable_sha256: String,
    head_executable_sha256: String,
    rustc: String,
    os: &'static str,
    arch: &'static str,
    cpu: Option<String>,
    kernel: Option<String>,
    repeats: usize,
    parallel_streams: usize,
    scope: &'static str,
    results: Vec<StreamReport>,
}

fn measure(
    build: Build<'_>,
    root: &Path,
    out: &Path,
    backend: &str,
    mode: &str,
    repeat: usize,
) -> Result<Run> {
    let output = Command::new(build.executable)
        .current_dir(root)
        .args(["--child", backend, mode])
        .env("MALLOC_ARENA_MAX", "1")
        .env("MALLOC_MMAP_THRESHOLD_", "131072")
        .env("MALLOC_TRIM_THRESHOLD_", "131072")
        .env("MALLOC_TOP_PAD_", "131072")
        .output()
        .with_context(|| format!("execute {}", build.executable.display()))?;
    let name = format!(
        "{backend}-{mode}-{repeat}-{}-{}",
        build.role, build.revision
    );
    std::fs::write(out.join(format!("{name}.json")), &output.stdout)?;
    std::fs::write(out.join(format!("{name}.stderr")), &output.stderr)?;
    ensure!(
        output.status.success(),
        "{name} failed with {}; logs preserved",
        output.status
    );
    let run: Run = serde_json::from_slice(&output.stdout).context("connected_idle output")?;
    ensure!(
        run.backend == backend && run.mode == mode && run.idle_seconds >= 61,
        "unexpected workload metadata for {name}"
    );
    Ok(run)
}

fn sample<'a>(run: &'a Run, checkpoint: &str) -> Result<&'a Sample> {
    run.samples
        .iter()
        .find(|sample| sample.checkpoint == checkpoint)
        .with_context(|| format!("missing checkpoint {checkpoint}"))
}

fn retained(run: &Run, checkpoint: &str) -> Result<i128> {
    Ok(i128::from(sample(run, checkpoint)?.rust_live_bytes)
        - i128::from(sample(run, "runtime_baseline")?.rust_live_bytes))
}

fn median(mut values: Vec<i128>) -> MedianBytes {
    assert!(!values.is_empty(), "median needs samples");
    values.sort_unstable();
    let middle = values.len() / 2;
    MedianBytes(if values.len().is_multiple_of(2) {
        values[middle - 1]
            .checked_add(values[middle])
            .expect("allocation samples fit twice the u64 range")
    } else {
        values[middle]
            .checked_mul(2)
            .expect("allocation samples fit twice the u64 range")
    })
}

fn compare(base: &[Run], head: &[Run], checkpoint: &str) -> Result<Comparison> {
    ensure!(
        !base.is_empty() && base.len() == head.len(),
        "matched nonempty repeats required"
    );
    for (base, head) in base.iter().zip(head) {
        ensure!(
            (
                base.lanes,
                base.messages,
                base.text_bytes,
                base.history_messages,
                base.workload_seed,
                base.idle_seconds
            ) == (
                head.lanes,
                head.messages,
                head.text_bytes,
                head.history_messages,
                head.workload_seed,
                head.idle_seconds
            ),
            "baseline and head workloads differ"
        );
    }
    let base_live = median(
        base.iter()
            .map(|run| retained(run, checkpoint))
            .collect::<Result<_>>()?,
    );
    let head_live = median(
        head.iter()
            .map(|run| retained(run, checkpoint))
            .collect::<Result<_>>()?,
    );
    let metrics = |runs: &[Run]| {
        runs.iter()
            .map(|run| {
                let sample = sample(run, checkpoint)?;
                Ok(SampleMetrics {
                    live: sample.rust_live_bytes,
                    peak: sample.rust_peak_bytes,
                    rss_anon_kib: sample.rss_anon_kib,
                })
            })
            .collect::<Result<Vec<_>>>()
    };
    Ok(Comparison {
        checkpoint: checkpoint.into(),
        base_retained_bytes: base_live,
        head_retained_bytes: head_live,
        delta_bytes: MedianBytes(head_live.0 - base_live.0),
        regression: head_live > base_live,
        base_samples: metrics(base)?,
        head_samples: metrics(head)?,
    })
}

pub struct Options {
    pub base_executable: PathBuf,
    pub head_executable: PathBuf,
    pub base_revision: String,
    pub head_revision: String,
    pub repeats: usize,
    pub out_dir: PathBuf,
}

pub fn run(root: &Path, options: Options) -> Result<()> {
    ensure!((3..=20).contains(&options.repeats), "use 3..=20 repeats");
    for revision in [&options.base_revision, &options.head_revision] {
        ensure!(
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "revision must be a full Git SHA"
        );
    }
    let base_executable = options.base_executable.canonicalize()?;
    let head_executable = options.head_executable.canonicalize()?;
    std::fs::create_dir_all(&options.out_dir)?;
    let out = options.out_dir.canonicalize()?;
    let results = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for backend in ["memory", "sqlite"] {
            for mode in ["control", "activity"] {
                let (base_executable, head_executable, out, options) =
                    (&base_executable, &head_executable, &out, &options);
                workers.push(scope.spawn(move || -> Result<StreamReport> {
                    let base_build = Build {
                        executable: base_executable,
                        revision: &options.base_revision,
                        role: "base",
                    };
                    let head_build = Build {
                        executable: head_executable,
                        revision: &options.head_revision,
                        role: "head",
                    };
                    let mut base = Vec::new();
                    let mut head = Vec::new();
                    for repeat in 0..options.repeats {
                        // Alternate order within each stream to expose temporal host drift.
                        if repeat % 2 == 0 {
                            base.push(measure(base_build, root, out, backend, mode, repeat)?);
                            head.push(measure(head_build, root, out, backend, mode, repeat)?);
                        } else {
                            head.push(measure(head_build, root, out, backend, mode, repeat)?);
                            base.push(measure(base_build, root, out, backend, mode, repeat)?);
                        }
                    }
                    Ok(StreamReport {
                        backend,
                        mode,
                        checkpoints: vec![
                            compare(&base, &head, "after_61s")?,
                            compare(&base, &head, "after_maintenance")?,
                        ],
                    })
                }));
            }
        }
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| anyhow::anyhow!("retention worker panicked"))?
            })
            .collect::<Result<Vec<_>>>()
    })?;
    let report = Report {
        schema_version: 2,
        base_revision: options.base_revision,
        head_revision: options.head_revision,
        base_executable_sha256: xtask_support::sha256(&std::fs::read(&base_executable)?),
        head_executable_sha256: xtask_support::sha256(&std::fs::read(&head_executable)?),
        base_executable,
        head_executable,
        rustc: String::from_utf8(xtask_support::capture(Command::new("rustc").arg("-Vv"))?.stdout)?,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        cpu: std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|info| {
                info.lines().find_map(|line| {
                    line.strip_prefix("model name")
                        .and_then(|value| value.strip_prefix("\t: "))
                        .map(str::to_owned)
                })
            }),
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .ok()
            .map(|value| value.trim().to_owned()),
        repeats: options.repeats,
        parallel_streams: 4,
        scope: "requested Rust heap after real connected idle; peak and Linux RSS are diagnostic; CPU/allocation work remains in CodSpeed",
        results,
    };
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let regressions = report
        .results
        .iter()
        .flat_map(|result| &result.checkpoints)
        .filter(|checkpoint| checkpoint.regression)
        .count();
    ensure!(
        regressions == 0,
        "{regressions} connected retention regressions; see {}",
        out.join("report.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run_with(live: u64, baseline: u64, messages: usize) -> Run {
        Run {
            backend: "memory".into(),
            mode: "control".into(),
            lanes: if messages == 0 { 0 } else { 32 },
            messages,
            text_bytes: 4096,
            history_messages: 0,
            workload_seed: 42,
            idle_seconds: 61,
            samples: vec![
                Sample {
                    checkpoint: "runtime_baseline".into(),
                    rust_live_bytes: baseline,
                    rust_peak_bytes: baseline,
                    rss_anon_kib: None,
                },
                Sample {
                    checkpoint: "after_61s".into(),
                    rust_live_bytes: live,
                    rust_peak_bytes: live,
                    rss_anon_kib: None,
                },
            ],
        }
    }
    #[test]
    fn a_one_byte_retention_regression_fails_even_when_absolute_heap_is_lower() {
        let result = compare(&[run_with(100, 10, 0)], &[run_with(92, 1, 0)], "after_61s").unwrap();
        assert_eq!(result.delta_bytes, MedianBytes(2));
        assert!(result.regression);
    }
    #[test]
    fn mismatched_workloads_cannot_pass_as_an_improvement() {
        assert!(
            compare(
                &[run_with(100, 10, 0)],
                &[run_with(50, 10, 256)],
                "after_61s"
            )
            .is_err()
        );
    }
    #[test]
    fn repeats_use_the_median_and_preserve_diagnostics() {
        let base = vec![
            run_with(100, 10, 0),
            run_with(100, 10, 0),
            run_with(100, 10, 0),
        ];
        let head = vec![
            run_with(99, 10, 0),
            run_with(99, 10, 0),
            run_with(200, 10, 0),
        ];
        let result = compare(&base, &head, "after_61s").unwrap();
        assert_eq!(result.delta_bytes, MedianBytes(-2));
        assert!(!result.regression);
        assert_eq!(result.head_samples.len(), 3);
    }
    #[test]
    fn even_repeat_medians_keep_half_bytes_exact() {
        assert_eq!(median(vec![90, 90, 91, 91]), MedianBytes(181));
        let value = serde_json::to_value(median(vec![90, 90, 91, 91])).unwrap();
        assert_eq!(value["numerator"], 181);
        assert_eq!(value["denominator"], 2);
    }

    #[test]
    fn changed_lane_count_is_not_a_valid_comparison() {
        let base = run_with(100, 10, 0);
        let mut head = run_with(50, 10, 0);
        head.lanes = 32;
        assert!(compare(&[base], &[head], "after_61s").is_err());
    }

    #[test]
    fn a_one_byte_regression_above_floating_point_precision_stays_visible() {
        let base = run_with(1_u64 << 60, 0, 0);
        let head = run_with((1_u64 << 60) + 1, 0, 0);
        let result = compare(&[base], &[head], "after_61s").unwrap();
        assert!(result.regression);
        assert_eq!(result.delta_bytes, MedianBytes(2));
    }
}
