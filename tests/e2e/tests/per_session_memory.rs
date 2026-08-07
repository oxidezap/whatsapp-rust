//! Per-session memory attribution: what one more connected client costs, and
//! how much of that cost `resource_report()` can name.
//!
//! `memory_soak.rs` answers "does a long-lived session grow?". This answers
//! "what does session N+1 add?", the number that decides which optimisation is
//! worth doing. It is deliberately marginal: the first client pays the whole
//! process-wide setup (crypto provider, lazily built statics, runtime threads),
//! which is ~28x the marginal cost and would swamp a total/N average.

use e2e_tests::TestClient;
use log::info;
use std::io::Read as _;

/// Read an env var as usize, falling back to the given default.
fn env_or(var: &str, default: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Current RSS in KiB from /proc/self/status (no page-size assumption), or
/// `None` where that file does not exist or carries no `VmRSS`. Absent RSS has
/// to stay distinguishable from zero RSS: folded to 0 it would silently turn
/// every delta into 0 and report an attribution of 0%.
fn rss_kib() -> Option<usize> {
    let mut buf = String::new();
    std::fs::File::open("/proc/self/status")
        .and_then(|mut f| f.read_to_string(&mut buf))
        .ok()?;
    buf.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.trim().strip_suffix("kB")?.trim().parse().ok()
    })
}

fn require_rss() -> anyhow::Result<usize> {
    rss_kib().ok_or_else(|| {
        anyhow::anyhow!("this harness measures RSS via /proc/self/status, which this host lacks")
    })
}

struct Session {
    index: usize,
    /// RSS growth this client caused, in bytes.
    rss_delta_bytes: u64,
    /// What `resource_report()` could attribute, in bytes.
    reported_bytes: u64,
    storage_bytes: Option<u64>,
    transport_bytes: Option<u64>,
    http_bytes: Option<u64>,
    client_bytes: u64,
}

fn median(mut values: Vec<u64>) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[values.len() / 2]
}

/// Connect `count` clients one at a time, keeping every one alive, and report
/// what each additional session costs against what the report can name.
///
/// Run with `--run-ignored all`; `SESSION_COUNT` overrides the client count.
#[tokio::test]
#[ignore = "measurement harness, run manually with --run-ignored all"]
async fn report_marginal_cost_per_session() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let count = env_or("SESSION_COUNT", 8);
    assert!(count >= 2, "a marginal cost needs at least two clients");

    let baseline_kib = require_rss()?;
    info!("=== PER-SESSION MEMORY: {count} clients, baseline RSS={baseline_kib}K ===");

    // Every client stays alive: dropping one would return its pages to the
    // allocator and turn the next delta into reuse rather than growth.
    let mut clients: Vec<TestClient> = Vec::with_capacity(count);
    let mut sessions: Vec<Session> = Vec::with_capacity(count);
    let mut previous_kib = baseline_kib;

    for index in 0..count {
        let client = TestClient::connect(&format!("permem_{index}")).await?;
        let report = client.client.resource_report().await;
        let now_kib = require_rss()?;

        sessions.push(Session {
            index,
            rss_delta_bytes: (now_kib.saturating_sub(previous_kib) * 1024) as u64,
            reported_bytes: report.total_estimated_bytes(),
            storage_bytes: report.storage.memory_bytes,
            transport_bytes: report.transport.map(|t| t.total_bytes()),
            http_bytes: report.http.map(|h| h.total_bytes()),
            client_bytes: report.client.total_estimated_bytes(),
        });
        previous_kib = now_kib;
        clients.push(client);
    }

    info!("  idx |  RSS delta |   reported |     client |    storage |  transport |       http");
    for s in &sessions {
        info!(
            "  {:>3} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10}",
            s.index,
            s.rss_delta_bytes,
            s.reported_bytes,
            s.client_bytes,
            s.storage_bytes.map_or("none".into(), |b| b.to_string()),
            s.transport_bytes.map_or("none".into(), |b| b.to_string()),
            s.http_bytes.map_or("none".into(), |b| b.to_string()),
        );
    }

    // The first client carries the process-wide setup, so it is reported but
    // never averaged in.
    let setup_bytes = sessions[0].rss_delta_bytes;
    let marginal = &sessions[1..];
    let marginal_rss = median(marginal.iter().map(|s| s.rss_delta_bytes).collect());
    let marginal_reported = median(marginal.iter().map(|s| s.reported_bytes).collect());
    let attributed_pct = if marginal_rss == 0 {
        0.0
    } else {
        marginal_reported as f64 * 100.0 / marginal_rss as f64
    };

    info!("--- attribution ---");
    info!("  one-time setup (client 0):  {setup_bytes} B");
    info!("  marginal RSS per session:   {marginal_rss} B (median of clients 1..{count})");
    info!("  attributed by the report:   {marginal_reported} B ({attributed_pct:.0}%)");
    info!(
        "  unattributed:               {} B",
        marginal_rss.saturating_sub(marginal_reported)
    );

    // The report documents itself as a lower bound on retained bytes, and RSS
    // carries allocator overhead on top (~1.1x live heap on glibc), so a report
    // above it means a component started inventing figures. Asserted on the
    // medians, not per client: one client's delta is rounded to KiB and can be
    // served from already-mapped pages, so a single quiet sample is normal.
    assert!(
        marginal_reported <= marginal_rss.saturating_mul(2),
        "median report {marginal_reported} B against {marginal_rss} B of median RSS growth; \
         total_estimated_bytes() is documented as a lower bound",
    );

    for client in clients {
        client.disconnect().await;
    }
    Ok(())
}
