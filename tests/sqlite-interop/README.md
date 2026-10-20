# Native SQLite interoperability host

This standalone consumer keeps the native graph from issue #1764: SDK
`sqlite-storage`, transport, runtime and HTTP opt-ins, with rusqlite 0.40.2.
The browser `js` feature stays disabled. No root or API-host lockfile is reused;
remove the generated lock to repeat a fresh downstream resolution.

The registry executes `bundled` and `sqlcipher` separately on native and MSRV.
The same test runs real SQL through rusqlite and account provisioning through
the SDK's Diesel-backed store. The SQLCipher profile asserts `cipher_version`
without configuring a key. All data is synthetic and held in memory.

Do not combine this workspace with the explicit browser host: Cargo considers
all workspace-member features when producing a lock, so that combined graph
would activate both incompatible `wsqlite3` requirements regardless of target.
