//! Fixed process cost versus per-session cost.
//!
//! `memory_soak.rs` asks whether a running session grows without bound. This
//! asks a different question: what does the *first* session cost that the
//! second does not, and how much of that is memory the library retains rather
//! than pages of its own binary faulted in on first execution.
//!
//! Both tests print a per-client table and a summary of first / second /
//! median-marginal deltas. Read the `anon` column, not `rss`: `file` is the
//! executable's own text and rodata being paged in, which is bounded by the
//! binary, shared by every session, and reclaimable.

use std::io::Read as _;
use std::sync::Arc;

use e2e_tests::TestClient;
use log::info;
use wacore::store::InMemoryBackend;
use whatsapp_rust::bot::Bot;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Resident memory split into its anonymous and file-backed halves.
#[derive(Debug, Clone, Copy, Default)]
struct Footprint {
    rss: i64,
    anon: i64,
    file: i64,
}

impl std::ops::Sub for Footprint {
    type Output = Footprint;

    fn sub(self, rhs: Footprint) -> Footprint {
        Footprint {
            rss: self.rss - rhs.rss,
            anon: self.anon - rhs.anon,
            file: self.file - rhs.file,
        }
    }
}

/// Reads `VmRSS` / `RssAnon` / `RssFile` from `/proc/self/status`, in KiB.
///
/// Panics rather than defaulting: a zero here would turn every delta into zero
/// and every upper-bound assertion into a pass, reporting a run that measured
/// nothing as a successful measurement.
fn footprint() -> Footprint {
    let mut buf = String::new();
    std::fs::File::open("/proc/self/status")
        .and_then(|mut f| f.read_to_string(&mut buf))
        .expect("/proc/self/status is required to measure a footprint");
    let read = |key: &str| -> i64 {
        for line in buf.lines() {
            if let Some(rest) = line.strip_prefix(key)
                && let Some(kb) = rest.trim().strip_suffix("kB").map(str::trim)
            {
                return kb
                    .parse()
                    .unwrap_or_else(|_| panic!("{key} is not a KiB count: {kb:?}"));
            }
        }
        panic!("/proc/self/status has no {key} field")
    };
    Footprint {
        rss: read("VmRSS:"),
        anon: read("RssAnon:"),
        file: read("RssFile:"),
    }
}

/// Fewer than three clients leaves no steady-state tail to take a marginal
/// from, so the assertions would pass on an empty sample.
fn client_count() -> usize {
    let Some(raw) = std::env::var("FOOTPRINT_CLIENTS").ok() else {
        return 16;
    };
    let n: usize = raw
        .parse()
        .unwrap_or_else(|_| panic!("FOOTPRINT_CLIENTS is not a number: {raw:?}"));
    assert!(n >= 3, "FOOTPRINT_CLIENTS must be at least 3, got {n}");
    n
}

/// Logs the per-client table and the first / second / median-marginal summary.
///
/// The median of clients 3..N is the marginal cost: client 2 still pays for
/// code paths the first client never reached, so averaging it in overstates
/// what a steady-state extra session costs.
fn report(label: &str, baseline: Footprint, steps: &[Footprint]) {
    info!("=== {label}: {} clients ===", steps.len());
    info!(
        "baseline                 rss={:>8} anon={:>8} file={:>8}",
        baseline.rss, baseline.anon, baseline.file
    );
    let mut prev = baseline;
    let mut deltas = Vec::with_capacity(steps.len());
    for (i, step) in steps.iter().enumerate() {
        let d = *step - prev;
        info!(
            "client {:>3}               rss={:>8} anon={:>8} file={:>8} | d_rss={:>+8} d_anon={:>+8} d_file={:>+8}",
            i + 1,
            step.rss,
            step.anon,
            step.file,
            d.rss,
            d.anon,
            d.file
        );
        deltas.push(d);
        prev = *step;
    }

    let median = |mut v: Vec<i64>| -> i64 {
        if v.is_empty() {
            return 0;
        }
        v.sort_unstable();
        v[v.len() / 2]
    };
    let tail = |pick: fn(&Footprint) -> i64| median(deltas.iter().skip(2).map(pick).collect());

    let first = deltas.first().copied().unwrap_or_default();
    let second = deltas.get(1).copied().unwrap_or_default();
    info!(
        "first client             rss={:>+8} anon={:>+8} file={:>+8}",
        first.rss, first.anon, first.file
    );
    info!(
        "second client            rss={:>+8} anon={:>+8} file={:>+8}",
        second.rss, second.anon, second.file
    );
    info!(
        "marginal (median 3..N)   rss={:>+8} anon={:>+8} file={:>+8}",
        tail(|d| d.rss),
        tail(|d| d.anon),
        tail(|d| d.file)
    );
}

/// Median anonymous growth per client over the steady-state tail.
fn marginal_anon(baseline: Footprint, steps: &[Footprint]) -> i64 {
    let mut prev = baseline;
    let mut deltas = Vec::new();
    for step in steps {
        deltas.push((*step - prev).anon);
        prev = *step;
    }
    let mut tail: Vec<i64> = deltas.into_iter().skip(2).collect();
    assert!(
        !tail.is_empty(),
        "no steady-state client to take a marginal from"
    );
    tail.sort_unstable();
    tail[tail.len() / 2]
}

/// What a `Client` costs before it has talked to anything.
///
/// Needs no mock server: `build()` performs no I/O, so the transport factory
/// points at a port nothing listens on.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn client_construction_footprint() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let n = client_count();
    let baseline = footprint();
    let mut bots = Vec::with_capacity(n);
    let mut steps = Vec::with_capacity(n);

    for i in 0..n {
        let bot = Bot::builder()
            .with_backend_arc(Arc::new(InMemoryBackend::new()))
            .with_transport_factory(
                whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory::new()
                    .with_url("wss://127.0.0.1:1/ws/chat"),
            )
            .with_http_client(UreqHttpClient::new())
            .with_runtime(whatsapp_rust::TokioRuntime)
            .with_version((2, 3000, 0))
            .with_push_name(format!("footprint_{i}"))
            .build()
            .await?;
        bots.push(bot);
        steps.push(footprint());
    }

    report("construction only", baseline, &steps);
    assert_eq!(bots.len(), n);

    // A built-but-unconnected client retains tens of KiB. The ceiling is an
    // order of magnitude above that, so it fires on a real per-client
    // regression and not on allocator noise.
    let marginal = marginal_anon(baseline, &steps);
    assert!(
        marginal < 1024,
        "marginal anonymous memory per constructed client is {marginal} KiB, over the 1 MiB ceiling"
    );
    Ok(())
}

/// What a connected, paired, synced session costs: the number a consumer
/// running several sessions in one process actually budgets against.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn session_footprint_fixed_vs_marginal() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let n = client_count();
    let baseline = footprint();
    let mut clients = Vec::with_capacity(n);
    let mut steps = Vec::with_capacity(n);

    for i in 0..n {
        clients.push(TestClient::connect(&format!("footprint_{i}")).await?);
        steps.push(footprint());
    }

    report("connected sessions", baseline, &steps);

    let marginal = marginal_anon(baseline, &steps);
    for client in clients {
        client.disconnect().await;
    }

    // Observed marginal is a few hundred KiB per session; the ceiling is wide
    // enough that only a genuine per-session leak trips it.
    assert!(
        marginal < 4096,
        "marginal anonymous memory per session is {marginal} KiB, over the 4 MiB ceiling"
    );
    Ok(())
}
