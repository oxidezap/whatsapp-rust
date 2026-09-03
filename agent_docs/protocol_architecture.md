# Protocol node architecture

Stanza builders and parsers live in `wacore/src/iq/`. Read this before adding a request type; the canonical worked examples are `wacore/src/iq/groups.rs` (rich: newtypes, nested children, enums) and `wacore/src/iq/blocklist.rs` (small).

## The two traits

- `ProtocolNode` (`wacore/src/protocol/mod.rs`) maps a struct to a node.
- `IqSpec` (`wacore/src/iq/spec.rs`) pairs a request with its typed response.

Both carry doc comments on every method; read the source rather than a copy here.

Non-obvious parts:

- **`try_from_node_ref(&NodeRef<'_>)` is the canonical parse path**, not the owned `try_from_node`. The owned form is a defaulted convenience that borrows and delegates. Implement and call the ref form.
- **`IqSpec` has an optional encode fast path** that writes the `<iq>` stanza straight into a pre-sized buffer and skips the `Node` intermediate. Returning `false` falls back to `build_iq()` + marshal, so it is safe to leave unimplemented — but do not add a second hand-rolled encoder next to it.
- **Constructors take `&Jid`**, never `Jid`, so callers are not forced to clone.
- **A response too large to hold as a tree implements `IqStreamSpec` too** (`wacore/src/iq/spec.rs`) and is sent with `client.execute_streaming(spec)`. The read loop then hands the spec a `wacore_binary::NodeStream` positioned inside the `<iq type="result">` and the spec walks the children one at a time (`open` descends, `next_child` decodes one child whole, `close` skips the rest), keeping only what it needs. `PropsSpec` is the worked example: the A/B catalog is ~2,700 `<prop>` children and a tree of it costs ~770 KB on a 64-bit host for a 30 KB frame, where the stream costs the 4 KB inflate window. `consume_response` is the only parser the client runs for such a spec: a result decoded whole for a raw-node observer is replayed to it over the tree's own node bytes (`OwnedNodeRef::backing_bytes`), and only an error stanza is reported from the tree. `parse_response` stays required (it is what `execute` returns) and should agree with it. `wacore/tests/props_stream_memory.rs` measures the frame's cost at the allocator and is the regression gate for it; `bench_props_catalog_{tree,stream}` in `wacore/binary/benches/binary_benchmark.rs` track the two readings' CPU cost on CodSpeed, which are at parity on a host.

## Derive macros

`wacore` re-exports exactly three, from `wacore-derive`:

| Derive | For |
| --- | --- |
| `EmptyNode` | Nodes that are only a tag |
| `ProtocolNode` | Nodes with attributes and children |
| `WireEnum` | Every protocol enum |

`StringEnum` is **not** a derive — it is an internal attribute kind inside the `ProtocolNode` derive. Protocol enums use `WireEnum`.

### `WireEnum` modes

The `#[wire = ...]` attribute is the single source of truth for a variant's wire value. Do not also derive serde or add `#[serde(rename_all)]` on these types — the derive owns both directions.

- **unit-string** (default) — `#[wire = "block"]` per variant.
- **int** — `#[wire(kind = "int")]` on the enum, `#[wire = 3]` per variant.
- **tagged with payload** — `#[wire(tag = "type")]` on the enum. Fields accept `#[wire_alias = "..."]` and `#[wire(skip)]`; `#[wire_fallback]` marks the catch-all variant, `#[wire_default]` the default.

Tagged mode generates a sibling `<Name>Tag` enum. **Parsers must dispatch through `<Name>Tag::try_from(node.tag.as_ref())`** instead of matching string literals, so renaming a wire tag stays a single-attribute change. The generator is `wacore/derive/src/lib.rs`.

The declarative `define_empty_node!` / `define_simple_node!` macros in `wacore/src/protocol/mod.rs` predate the derives. Do not reach for them in new code.

## Parsing helpers

`wacore/src/iq/node.rs` holds crate-internal helpers: `required_child`, `optional_child`, `required_attr`, `optional_attr`, `collect_children`, `extract_content_bytes`, `extract_content_uint`. They exist so error messages about missing tags and attributes stay uniform — prefer them over open-coding `get_optional_child` with a bespoke `anyhow!`.

## Validated newtypes

Protocol limits belong in the type, not in a check at the call site. `GroupSubject` in `wacore/src/iq/groups.rs` is the pattern. The limits themselves (`GROUP_SUBJECT_MAX_LENGTH`, `GROUP_DESCRIPTION_MAX_LENGTH`, `GROUP_SIZE_LIMIT`) come from WhatsApp Web's A/B props registry — confirm one against the registry before changing it, in one command; see `wa_web_reference.md`.

## File layout

One file per feature under `wacore/src/iq/`, each holding its constants, enums, request/response structs, `IqSpec` impls, and unit tests. `mod.rs` re-exports, `spec.rs` and `node.rs` are shared infrastructure.
