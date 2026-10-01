//! Standalone process-isolated, real elapsed connected-session retention probe.
//! Build first; run this executable directly. See `benches/connected_idle.md`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

#[path = "connected_idle/allocator.rs"]
mod allocator;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use whatsapp_rust::bench_support::connected_idle::{
    BackendFixture, Checkpoint, HISTORY_MESSAGES, IDLE, LANES, MESSAGES, Session, TEXT_BYTES,
    WORKLOAD_SEED,
};

#[global_allocator]
static ALLOC: allocator::Counting = allocator::Counting;

#[derive(Debug, Serialize, Deserialize)]
struct Sample {
    checkpoint: String,
    elapsed_ms: u128,
    rust_live_bytes: usize,
    rust_peak_bytes: usize,
    rss_anon_kib: Option<i64>,
    #[serde(skip_deserializing)]
    lifecycle: Option<Checkpoint>,
    pongs: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct Run {
    backend: String,
    mode: String,
    pid: u32,
    lanes: usize,
    messages: usize,
    text_bytes: usize,
    history_messages: usize,
    workload_seed: u64,
    idle_seconds: u64,
    samples: Vec<Sample>,
}

#[cfg(target_os = "linux")]
fn rss_anon_kib() -> Result<Option<i64>> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("RssAnon:"))
        .context("/proc/self/status has no RssAnon field")?;
    let number = line
        .trim()
        .strip_suffix("kB")
        .context("RssAnon unit is not kB")?
        .trim();
    Ok(Some(number.parse()?))
}

#[cfg(not(target_os = "linux"))]
fn rss_anon_kib() -> Result<Option<i64>> {
    Ok(None)
}

async fn sample(label: &str, start: Instant, session: Option<&Session>) -> Result<Sample> {
    let lifecycle = if let Some(session) = session {
        let checkpoint = session.checkpoint().await;
        ensure!(checkpoint.connected, "{label}: session disconnected");
        Some(checkpoint)
    } else {
        None
    };
    memory_sample(label, start, lifecycle, session.map_or(0, Session::pongs))
}

fn memory_sample(
    label: &str,
    start: Instant,
    lifecycle: Option<Checkpoint>,
    pongs: usize,
) -> Result<Sample> {
    // Label allocation happens before the baseline/live read for every row. The
    // /proc read is dropped before sampling live bytes and its transient is not
    // included in the sampled peak (which is captured first).
    let checkpoint = label.to_owned();
    let rust_peak_bytes = allocator::peak();
    let rss_anon_kib = rss_anon_kib()?;
    let rust_live_bytes = allocator::live();
    Ok(Sample {
        checkpoint,
        elapsed_ms: start.elapsed().as_millis(),
        rust_live_bytes,
        rust_peak_bytes,
        rss_anon_kib,
        lifecycle,
        pongs,
    })
}

// The native probe must verify actual elapsed time, not a pluggable test clock.
#[allow(clippy::disallowed_methods)]
fn child(backend: &str, mode: &str) -> Result<Run> {
    ensure!(
        matches!(backend, "memory" | "sqlite"),
        "backend must be memory or sqlite"
    );
    ensure!(
        matches!(mode, "activity" | "control"),
        "mode must be activity or control"
    );
    let start = Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut run = rt.block_on(async {
        let mut samples = Vec::with_capacity(7);
        allocator::reset_peak();
        samples.push(sample("runtime_baseline", start, None).await?);
        let store = if backend == "sqlite" {
            BackendFixture::sqlite().await?
        } else {
            BackendFixture::memory()
        };
        let session = Session::connect(store.backend()).await?;
        samples.push(sample("connected_before_activity", start, Some(&session)).await?);
        if mode == "activity" {
            let activity = session.prepare_activity().await?;
            session.receive_activity(activity).await?;
        } else {
            session.finish_control().await?;
        }
        samples.push(sample("after_activity", start, Some(&session)).await?);
        let idle_start = Instant::now();
        session.idle().await?;
        ensure!(
            idle_start.elapsed() >= IDLE,
            "native path must wait real elapsed time"
        );
        samples.push(sample("after_61s", start, Some(&session)).await?);
        session.maintenance().await?;
        samples.push(sample("after_maintenance", start, Some(&session)).await?);
        let idle = samples[3].lifecycle.context("idle lifecycle missing")?;
        ensure!(
            idle.open_lanes == 0 && idle.running_workers == 0,
            "idle workers did not stop"
        );
        ensure!(
            session.pongs() > 0,
            "production keepalive never reached the server during real idle"
        );
        ensure!(
            idle.messages == if mode == "activity" { MESSAGES } else { 0 },
            "delivery mismatch"
        );
        session.shutdown().await?;
        store.cleanup()?;
        samples.push(sample("after_shutdown", start, None).await?);
        Ok::<_, anyhow::Error>(Run {
            backend: backend.into(),
            mode: mode.into(),
            pid: std::process::id(),
            lanes: if mode == "activity" { LANES } else { 0 },
            messages: if mode == "activity" { MESSAGES } else { 0 },
            text_bytes: TEXT_BYTES,
            history_messages: if mode == "activity" {
                HISTORY_MESSAGES
            } else {
                0
            },
            workload_seed: WORKLOAD_SEED,
            idle_seconds: IDLE.as_secs(),
            samples,
        })
    })?;
    drop(rt);
    run.samples
        .push(memory_sample("after_runtime_drop", start, None, 0)?);
    Ok(run)
}

fn range(values: &mut [i64]) -> (i64, i64, i64) {
    values.sort_unstable();
    (
        values[0],
        values[values.len() / 2],
        values[values.len() - 1],
    )
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--child") {
        ensure!(
            args.len() == 3,
            "internal child usage: --child <memory|sqlite> <activity|control>"
        );
        let run = child(&args[1], &args[2])?;
        serde_json::to_writer(std::io::stdout().lock(), &run)?;
        println!();
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--help") {
        println!(
            "connected_idle [--repeats N]\nRuns fresh activity/control children for memory and real SQLite; 61s idle each. Default 3 repeats (~12min). JSONL samples on stdout, min/median/max deltas on stderr. Run from repository root. RssAnon is Linux-only; Rust heap excludes C allocations."
        );
        return Ok(());
    }
    let repeats = if args.is_empty() {
        3
    } else {
        ensure!(
            args.len() == 2 && args[0] == "--repeats",
            "usage: connected_idle [--repeats N]"
        );
        args[1].parse::<usize>()?
    };
    ensure!((1..=20).contains(&repeats), "repeats must be 1..=20");
    if repeats < 3 {
        eprintln!("Only {repeats} repeat(s): smoke check, not a noise estimate.");
    }
    #[cfg(not(target_os = "linux"))]
    eprintln!("RssAnon unsupported on this platform: JSON reports null, never zero.");
    let executable = std::env::current_exe()?;
    for backend in ["memory", "sqlite"] {
        for mode in ["control", "activity"] {
            let mut runs = Vec::with_capacity(repeats);
            for _ in 0..repeats {
                // No test runner, Cargo process, sibling sample or server shares the
                // child's address space. Sequential repeats bound VPS resource use.
                let output = std::process::Command::new(&executable)
                    .args(["--child", backend, mode])
                    .output()?;
                ensure!(
                    output.status.success(),
                    "{backend}/{mode} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let run: Run = serde_json::from_slice(&output.stdout)?;
                print!("{}", std::str::from_utf8(&output.stdout)?);
                runs.push(run);
            }
            for index in 2..7 {
                let mut heap: Vec<i64> = runs
                    .iter()
                    .map(|run| {
                        run.samples[index].rust_live_bytes as i64
                            - run.samples[0].rust_live_bytes as i64
                    })
                    .collect();
                let heap = range(&mut heap);
                let anon: Option<Vec<i64>> = runs
                    .iter()
                    .map(|run| {
                        Some(run.samples[index].rss_anon_kib? - run.samples[0].rss_anon_kib?)
                    })
                    .collect();
                eprintln!(
                    "{backend}/{mode}/{} delta from runtime baseline min/median/max: Rust live B={heap:?}, RssAnon KiB={:?}",
                    runs[0].samples[index].checkpoint,
                    anon.map(|mut values| range(&mut values))
                );
            }
        }
    }
    Ok(())
}
