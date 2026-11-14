# Normal and xpop relay candidates

Follow-up to [#1912](https://github.com/oxidezap/whatsapp-rust/pull/1912).
When a `te2` carries both `token_id=1` and `xtoken_id=2`, they describe two
candidate identities. Treating `xtoken_id` only as a spelling fallback discards
the second candidate even when token 1 was never supplied or its slot is empty.

`parse_relay_candidates` retains `(relay_id, Normal)` and `(relay_id, Xpop)`
separately, including when both use the same token. The existing `RelayData`
interface projects both endpoints without adding required public struct fields.
The candidate API additionally exposes xpop RTT (default 100 ms) and maximum
peer c2r RTT. A same-identity token conflict rejects the block; same-family and
same-protocol addresses are updated, and other address slots remain intact.
Names are metadata, not identity. Outbound enumeration no longer collapses
normal/xpop entries by relay name.

Selection retains the existing Rust tiers and ordinary-token preference within
a dual `te2`. Usability checks can now select xpop when the normal token is absent
or empty. This is a compatibility policy, not the native engine's election
algorithm. TCP metadata is retained, while the current UDP transport uses only
protocol 0 addresses. Protocols outside native UDP/TCP are ignored. Credential
buffers remain inside the redacted `RelayData`; `RelayCandidate` contains indices
and addresses, never token/key bytes.

## Source and static evidence

Snapshot: [whatspec ae3880925bacd23863cc30ec480c9f82bfd869f1](https://github.com/oxidezap/whatspec/tree/ae3880925bacd23863cc30ec480c9f82bfd869f1),
WA 2.3000.1052559453. The [snapshot lock](https://github.com/oxidezap/whatspec/blob/ae3880925bacd23863cc30ec480c9f82bfd869f1/generated/wasm.lock.json)
identifies `u-Bw8hwXJGE.wasm` (handle 110937), 11,492,008 bytes,
SHA-256 `c73edd705435d0d22b19fd3817c9bed536d8bc6a5fbe8a927cbd277fa26fff4f`.
The handle 32180 filename `jXNNwxgnc5P.wasm` has identical bytes.

The public [WASM archive](https://github.com/oxidezap/whatspec/releases/download/bundle-store/wasm-905d7c6b0b4769fd8bbd3d8860d9ef4e415873c65a00b85573461549695a2375.tar.xz)
has SHA-256 `cb9996956dc029eed817640d7c750bb3e833fe6fb1bc1bb4d1ead7ddf3f82335`.
The archive set hash is `905d7c6b0b4769fd8bbd3d8860d9ef4e415873c65a00b85573461549695a2375`.
All function numbers below apply only to this exact capture.

Use [unwasm 72844fd8848358e23abbeae232abf9b3d68817b2](https://github.com/oxidezap/unwasm/tree/72844fd8848358e23abbeae232abf9b3d68817b2)
with level 0 for faithful translation:

```sh
unwasm decompile u-Bw8hwXJGE.wasm --only 513,14166,14350,14381,11856,11855,14164,5464,12066,12067,12068,11938,12021 --level 0 --offsets -o relay-evidence
unwasm table u-Bw8hwXJGE.wasm
```

| Stage | Pinned native evidence | Rust behavior / boundary |
| --- | --- | --- |
| Parse attributes | f513 reads `token_id` at file offsets 162302/162334 and `xtoken_id` at 162366. f14166 reads both separately; xpop insertion at 6151617, ordinary insertion at 6151817. Xpop does not require explicit `xrtt_ms`; default is 100. | Two entries retained; missing/malformed nonnegative token attributes preserve #1912 fallback/default semantics. |
| Identity / updates | f14350 (`wa_call_add_or_update_relay_item`, table slot 10165), body bytes `[6265707,6266428)`, encoded-body SHA-256 `9a967455d24c06a19e63bd9dd785fd24259cfa74abe028bfd9e9089ced5297dc`. Compares relay id and xpop flag, returns 70004 on token conflict. Stores token at item+1, xpop at +52, xRTT at +56, c2r at +60, max-peer c2r at +64. | Same identity key and address update rule; conflicting bindings reject parsing. |
| Native limits | f14350 has eight entries of 164 bytes and two 24-byte protocol address slots. Full list returns 70020. Token and relay ids are stored as bytes. | Existing indexed-token bound remains 64. Rust does not emulate byte truncation or impose native eight-entry storage. Unknown/missing token indices remain unusable and do not cause indexed allocations. |
| Token collection | f14164 collects at most eight tokens, each up to 512 bytes, in positional order. | Existing sparse token-node `id` indexing remains. This PR does not assert sparse/malformed token-node parity with every native parser. |
| Transport binding | f14381 reads each item's stored token index at 6280917 and xpop flag at 6280968, passes both to f11856 at 6281003. f11856 stores token bytes/length at context+132/+644, xpop/+2010 and RTT fields/+2012, +2016, +2020; rejects a nonpositive token length. | Both endpoints survive parsing and selection checks their own token. MediaSessionSpec → engine config → Allocate uses the selected token. Full parallel multi-candidate transport state is not implemented. |
| Authentication | f11855 selects valid auth id including 0; missing/out-of-range auth logs a fallback to relay token. | Existing separate auth-token selection is retained. Browser auth fallback is a remaining difference; unrelated to which token signs Allocate. |
| Election | f12066/f12067 inspect reachability, connected peer count, gated UDP preference, latency sums and hysteresis. f12068 can switch normal/xpop during a call. f11938 conditionally weights xpop latency with xrtt via config+972/+976. | Existing static usable-endpoint policy remains. Metadata is exposed but no speculative RTT ranking, racing or retry policy is added. |
| Web serialization | f5464 emits stored item+1 as JSON `token_id` (offsets 2207400/2207425) and auth id as `auth_token_id` (2207541). | Absence of literal `xtoken_id` in a JS consumer cannot establish absence of native support. |

The preserved JS bundle SHA-256 `79fb7fad8cd23aede9167f0eac55525db0796e9087d0d965624781b97e66ee2c` contains
`WAWebVoipRelayConnectionUtils` at byte range `[15169,20462)`,
`WAWebVoipSctpConnectionManager` at `[77758,111555)` and
`WAWebVoipSctpWarmStandby` at `[67940,73789)`. The full snapshot has 676 locked
JS bundles and set hash `d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf`.
The relay utility consumes normalized `token_id`, bounds-checks it and only
creates protocol-0 connection entries. The manager can start multiple connections.
Warm standby uses either the advertised port or 3480 according to its gate;
connected callbacks can notify the native engine through f1195/f12021.
These are static source observations, not proof that a gate is enabled for a
particular account. The Rust native port race is within one selected host and
must not be described as equivalent to Web's multi-relay election.

## Executable oracle and regressions

The additive pin in `tools/oracle-core/wasm.lock.json` keeps previous captures.
No WASM binary is committed. The native test validates the capture and exact
insertion-body selector before using table slot 10165. It initializes synthetic
bounded scratch memory without constructors or live calling state. Logging is
explicitly disabled with threshold -1 at capture-specific address 1427744:
f4600 (`pj_log_internal`, body `[2009849,2011541)`, SHA-256
`5a25bc62ee999fcf7bc13524767a5129548a168e88c28702b87155b19efff86e`)
then returns before touching logger TLS. The test verifies that selector too.
This isolates the relay primitive; it does not validate engine initialization.
It checks:

- separate normal/xpop identities, including equal token ids;
- conflict 70004 and full-list 70020 without changing the list;
- same-family address replacement preserving the other family.

Tests require zero recorded import calls and reject every invoked unsupported
stub. Counters for recorded calls are not a guarantee that every implemented
import is instrumented. An independent run in the original Node WebAssembly
runtime, with every function import set to throw, also exercised success,
conflict and capacity paths with logging disabled and reached no imports.

```sh
cargo xt oracle fetch
WA_WASM_DIR="$PWD/.cache/wa-wasm" cargo test --release -p oracle-core --test relay_candidates -- --nocapture
cargo test -p wacore --features voip-control relay_parse
cargo test -p wacore --features voip xpop_selection_keeps_credentials_bound_through_media_setup
cargo fmt --all -- --check
```

The media-setup regression checks the selected endpoint, relay-token and auth-token
(including auth index 0), then inspects actual engine Allocate attribute 0x4000.
It covers both a usable normal token and a missing normal token with usable xpop.
The parser tests also cover missing, malformed, negative, empty, sparse and
out-of-set token references. Existing unsigned parsing is retained; the captured
C++ integer parser accepts signed i32 and rejects values above 2147483647, which
is a documented remaining difference for malformed/out-of-domain attributes.

No authenticated live server Allocate, media reception, runtime gate state,
full dynamic relay election or performance improvement is established by these
synthetic tests. The author's observed 452 reason was `Xor Relayed Address
Mismatch`; this work does not establish a universal cause for every 452.
