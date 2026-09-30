# A10: canonical downloads and final failures

Use the same entry points for messages and persisted CDN fields:

```rust,no_run
# async fn example(client: &whatsapp_rust::Client, params: &whatsapp_rust::download::DownloadParams) -> anyhow::Result<()> {
let bytes = client.download(params).await?;
let writer = client
    .download_to_writer(params, std::io::Cursor::new(Vec::new()))
    .await?;
# let _ = (bytes, writer);
# Ok(()) }
```

`download_from_params(params)` and `download_from_params_to_writer(params, writer)`
remain delegating aliases, now deprecated. Replace them with `download(params)`
and `download_to_writer(params, writer)`. Removal is reserved for a future
breaking release; there is no additional entry point per source type.

## Final errors

Client downloads (including the aliases) now return
`whatsapp_rust::download::ClientDownloadError`, not `anyhow::Error`:

- `ReferenceRejected`: CDN rejection after the applicable refresh budget.
- `HostsUnreachable`: all hosts failed; the last transport/status/integrity cause.
- `NoHosts`: the route contained no hosts; no HTTP exchange took place.
- `NoHostsAfterRefresh`: a CDN rejection triggered a forced refresh, but the
  refreshed route contained no hosts. The prior rejection remains the source,
  including its HTTP status; this is not mistaken for a never-attempted route.
- `Preparation`: the reference could not be turned into a request.
- `MediaSession { force_refresh, source }`: session acquisition (`false`) or
  refresh after rejection (`true`) failed, retaining the original `IqError`.
- `WriterCleanup { failure, cleanup }`: the download failed and clearing the
  sink also failed. Inspect `failure` for the original final classification and
  `cleanup` for the I/O cause. The standard source chain follows `failure`.

The enum is non-exhaustive: include a wildcard arm. Existing `anyhow::Result`
application functions can still use `?`; explicit assignments/returns of an
`anyhow::Error` need `.into()` or `.map_err(Into::into)`. Instead of calling
`downcast_ref` on the Client result, match the variant or inspect its source
chain with `ErrorChainExt`. HTTP status, existing typed decryption failures and
session IQ rejection metadata remain recoverable in that chain. The existing
streaming crypto path reports a MAC failure as an `anyhow` message cause, whereas
buffered verification supplies `MediaDecryptionError::InvalidMac`; A10 preserves
both without changing crypto or inventing a uniform typed integrity contract.

`MediaDownloader` remains independent of Client sessions, returns
`MediaDownloadError`, and never refreshes a session. Its existing variants are
preserved; `WriterCleanup { failure, cleanup }` is new. Preparation remains
`Other` on this legacy independent error API.

Static URLs use the URL verbatim without a media-connection IQ. A rejected
static URL is final: no invalidation, wasted IQ, or repetition of the same URL.
Non-static references retain the existing refresh budget and host failover.
Failures obtaining/refreshing a session are not mislabeled as CDN failures.

## Writer integrity and costs

Success returns the writer rewound, containing **exactly authenticated media**.
Each streaming attempt truncates before writing, so a shorter successful retry
cannot retain the previous attempt's tail. Existing destination contents are
replaced, not preserved. On an ordinary returned error, the consumed writer is
cleared and rewound if the sink allows it. If cleanup fails, `WriterCleanup`
reports that fact; do not interpret the retained sink as valid media. Cancellation,
panic, or runtime shutdown can prevent a returned result/cleanup. A caller with a
separate file/handle must not publish or read the output until success.

Buffered HTTP downloads reuse the response allocation for in-place verification
and decryption; no second full plaintext buffer is introduced. Streaming writers
retain bounded crypto/read state, not a file-sized `Vec`. A non-streaming HTTP
adapter necessarily buffers the response even for writer calls. The buffer and
writer APIs retain distinct costs and ownership. No crypto, upload contract,
HTTP configuration, or host trait sealing change is part of A10.

Tests: `src/download.rs` invariants plus `tests/api_a10_download.rs`. The same
external API fixture is a standalone package at
`tests/fixtures/api_a10_download`; it can be run from outside the workspace to
avoid inheriting `.cargo/config.toml`. Workspace CI covers the shared API test
source, not this separate manifest. Validate the latter explicitly from outside
the workspace, for example:

```sh
cd /tmp
CARGO_TARGET_DIR=/path/to/task-target cargo test \
  --manifest-path /path/to/whatsapp-rust/tests/fixtures/api_a10_download/Cargo.toml
```
