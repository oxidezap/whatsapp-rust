#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use serde_json::json;

fn output(command: &mut Command) -> String {
    let output = command.output().expect("execute measurement command");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 command output")
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "expected ORACLE CANDIDATE OUTPUT_DIRECTORY");
    let directory = Path::new(&args[3]);
    std::fs::create_dir_all(directory).expect("create evidence directory");
    let mut samples: BTreeMap<String, [Vec<f64>; 2]> = BTreeMap::new();
    // Fix the decision rule before collecting results. The report retains
    // every scenario and marks unstable measurements inconclusive.
    let repetitions = 9;
    let iterations = 2_000_000;
    for repeat in 0..repetitions {
        for index in if repeat % 2 == 0 { [0, 1] } else { [1, 0] } {
            let raw = output(Command::new(&args[index + 1]).arg(iterations.to_string()));
            std::fs::write(directory.join(format!("sample-{repeat}-{index}.tsv")), &raw)
                .expect("write raw sample");
            for line in raw.lines() {
                let mut fields = line.split('\t');
                let name = fields.next().expect("scenario");
                let count: u64 = fields
                    .next()
                    .expect("iterations")
                    .parse()
                    .expect("iteration count");
                let nanos: u128 = fields.next().expect("time").parse().expect("elapsed time");
                assert_eq!(count, iterations);
                assert!(fields.next().is_none());
                samples.entry(name.into()).or_default()[index].push(nanos as f64 / count as f64);
            }
        }
    }
    assert_eq!(
        samples.len(),
        36,
        "every probe must emit all workload operations"
    );
    let mut scenarios = BTreeMap::new();
    for (name, values) in samples {
        assert!(values.iter().all(|v| v.len() == repetitions));
        let medians = [median(&values[0]), median(&values[1])];
        let mad = [0, 1].map(|index| {
            median(
                &values[index]
                    .iter()
                    .map(|v| (v - medians[index]).abs())
                    .collect::<Vec<_>>(),
            ) / medians[index]
        });
        let delta = medians[1] / medians[0] - 1.;
        let verdict = if mad.iter().any(|&v| v > 0.03) {
            "inconclusive-noise"
        } else if delta > 0.05 {
            "regression"
        } else if delta < -0.05 {
            "improvement"
        } else {
            "within-tolerance"
        };
        scenarios.insert(name, json!({ "oracle_ns": medians[0], "candidate_ns": medians[1], "delta_percent": delta * 100., "mad_fraction": mad, "verdict": verdict, "samples_ns": values }));
    }
    let metadata = json!({
        "head": output(Command::new("git").args(["rev-parse", "HEAD"])).trim(),
        "compiler": output(Command::new("rustc").arg("-Vv")),
        "oracle_sha256": output(Command::new("sha256sum").arg(&args[1])).split_whitespace().next(),
        "candidate_sha256": output(Command::new("sha256sum").arg(&args[2])).split_whitespace().next(),
        "oracle_bytes": std::fs::metadata(&args[1]).expect("oracle size").len(),
        "candidate_bytes": std::fs::metadata(&args[2]).expect("candidate size").len(),
        "profile": "release, workspace defaults",
        "allocator": "system, no custom allocator",
        "warmup_iterations": 10_000,
        "iterations": iterations,
        "repetitions": repetitions,
        "noise_mad_fraction_limit": 0.03,
        "individual_regression_fraction_limit": 0.05,
        "scenarios": scenarios,
    });
    std::fs::write(
        directory.join("report.json"),
        serde_json::to_vec_pretty(&metadata).expect("serialize report"),
    )
    .expect("write report");
    writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&metadata).expect("serialize report")
    )
    .expect("write report");
}
