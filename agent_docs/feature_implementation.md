# Finding a feature's wire format

The wire format is the ground truth, and it is discovered, not designed. Everything else — layering, ergonomics, tests — follows from it. This doc covers reading the raw bundle; conventions live in `AGENTS.md`, stanza mechanics in `protocol_architecture.md`.

**Try the structured IR first.** `wa_web_reference.md` covers whatspec, which already extracts stanza shapes, enums, protocol limits, and notification dispatch into queryable JSON. Come back here when it can't answer — when the question is about control flow, ordering, or *when* a request is sent rather than what it contains.

## Where to look

`docs/captured-js/` is a local, untracked dump of WhatsApp Web (~3200 files under `WA/Smax/` alone). It is the highest-fidelity source available; whatsmeow and Baileys are second opinions when it is ambiguous.

Navigation that actually works:

- **`WA/Smax/Out*.js`** — outgoing request builders. `OutGroupsAddParticipantsRequest.js` is the shape of the stanza the client sends.
- **`WA/Smax/In*.js`** — incoming response parsers, usually split into a success file and several error files per operation. The error files are where the server's failure taxonomy is written down.
- Filenames repeat with a `__<hash>` suffix across bundle versions. Read the unsuffixed one; diff against a suffixed one only if you suspect the behavior changed between captures.
- **`exports-map.json`** and **`dep-graph.json`** resolve a symbol to its module and find callers, which is faster than grepping 3000 files.
- **`metadata.json`** lists GK gates — useful when a code path looks dead and is really feature-flagged.

Patterns worth grepping across the dump: `xmlns:` for namespaces, `action:` for action attributes, `smax("tag", { attrs })` for node construction.

## Reading evidence honestly

Reading the JS gives you a hypothesis. If a capture yields an algorithm — a hash, a key derivation, a constant — run it against real captured data and report the hit rate against chance before building on it. "6 of 113 matched, 0.012 expected by chance" is evidence; "the code says md5" is a reading.

The capture stays local, and only the aggregate leaves it: report the rate, never the rows. Real JIDs, phone numbers, and vectors derived from them do not belong in tests, commits, PR bodies, or issues — regenerate fixtures from fictitious identifiers once the hypothesis holds.

## Which crate

- **wacore** — protocol logic, state traits, crypto helpers, data models. Platform-agnostic: it also builds for wasm32 and ESP32.
- **whatsapp-rust** — runtime orchestration, storage, user-facing API.
- **waproto** — protobuf structures only.

If a feature seems to need Tokio inside `wacore`, the split is wrong: the runtime-dependent half belongs in `whatsapp-rust`.

## Order of construction

Build the smallest builder that round-trips against the real server, and parse the response path before adding options or convenience. An ergonomic API built on an unverified stanza shape has to be rebuilt. Logging the client's own traffic next to a WA Web capture is the cheapest confirmation you get.

## Map

- Protocol entry points: `src/send.rs`, `src/message.rs`, `src/socket/`, `src/handshake.rs`
- Feature modules: `src/features/`
- State and storage: `src/store/` + `PersistenceManager`
- Core protocol and crypto: `wacore/`
- Protobufs: `waproto/`
