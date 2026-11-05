use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

fn main() -> Result<std::process::ExitCode> {
    let mut args = std::env::args_os().skip(1);
    let command = args
        .next()
        .context("usage: mobile-oracle verify <lock> <root> | diff <before-lock> <after-lock> | compare <lock> <root> <corpus> <apk-observations> <rust-observations>")?;
    let first = PathBuf::from(args.next().context("missing first path")?);
    let second = PathBuf::from(args.next().context("missing second path")?);
    let (output, passed) = if command == "compare" {
        let corpus = PathBuf::from(args.next().context("missing corpus path")?);
        let apk = PathBuf::from(args.next().context("missing APK observations path")?);
        let rust = PathBuf::from(args.next().context("missing Rust observations path")?);
        if args.next().is_some() {
            bail!("unexpected argument");
        }
        let report = mobile_oracle::compare_observations(&first, &second, &corpus, &apk, &rust)?;
        let passed = report.status == mobile_oracle::ComparisonStatus::Pass;
        (serde_json::to_string_pretty(&report)?, passed)
    } else {
        if args.next().is_some() {
            bail!("unexpected argument");
        }
        let report = if command == "verify" {
            serde_json::to_string_pretty(&mobile_oracle::verify(&first, &second)?)?
        } else if command == "diff" {
            serde_json::to_string_pretty(&mobile_oracle::compare_locks(&first, &second)?)?
        } else {
            bail!("unknown command; expected verify, diff or compare");
        };
        (report, true)
    };
    writeln!(std::io::stdout().lock(), "{output}")?;
    Ok(std::process::ExitCode::from(if passed { 0 } else { 2 }))
}
