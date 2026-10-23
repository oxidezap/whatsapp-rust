# SDK-only SQLite browser host

This standalone host selects `whatsapp-rust` with `sqlite-storage` and `js`.
It has no direct dependency on storage, wacore, or sqlite-wasm-rs that could
mask a missing SDK feature edge. Its lockfile is generated independently of
the root and general API host locks; remove it to repeat a fresh resolution.

The consumer registry runs native/MSRV construction tests and executes the
WASM test in a real headless browser through wasm-bindgen-test-runner. CI uses
the hosted image's matching Chrome/ChromeDriver pair. The exported `probe`
also permits local browser harnesses to execute the identical operation.

The probe runs migrations, creates two synthetic device rows, and reads them
through the public SDK re-export. The database uses the memory VFS. It does
not test OPFS persistence or connect to WhatsApp.

Keep this resolve separate from `tests/sqlite-interop`: activating the browser
0.6 edge beside rusqlite 0.40's 0.5 requirement recreates the links conflict,
even for a native target.
