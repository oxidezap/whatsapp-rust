# Layout asserts and how to rebaseline them

Several tests pin `size_of` values so a careless field addition shows up as a
test failure instead of silent memory growth. Those pins break for innocent
reasons too. A dependency upgrade reshapes an embedded struct, a compiler
release repacks padding, and the assert fires with no real regression behind
it. This file lists every pin, says which ones are exact and which are bounds,
and gives the steps to reset one honestly.

## Policy

Exact equality stays only where the value is a contract. A packed struct whose
bytes hit disk, an enum whose width multiplies per string, a queue that must
hold exactly two handles. Everywhere else the assert is a budget, and budgets
use `<=`. A smaller struct is never a failure.

Pointer width gets a `cfg` gate, never a fudge factor. Where the property can
be stated without numbers at all, prefer that. `2 * size_of::<usize>()`,
`size_of::<Agent>() + N`, or `4 * size_of::<usize>()` survive a width change
and a repack that a bare number does not. #1478 and #1479 set this pattern.

## Inventory

Exact, kept exact:

- `DeviceInfo == 8` in `wacore/src/store/traits.rs`. The fields are `u16 +
  u8 + u32`, seven bytes, so the 8-byte size includes one padding byte.
  Growth means the packing broke.
- `StringHint == 5`, `ParsedJidMeta == 5` in `wacore/binary/src/encoder.rs`.
  The hint tape stores one entry per string in the payload, so each byte
  multiplies across every string. A wider entry needs justification.
- `QueuedChatMessage == 2 * size_of::<usize>()` in
  `src/handlers/message.rs`. Compositional, already width independent. The
  queue entry must stay two handles.
- The four-word saving in `wacore/libsignal/src/protocol/sender_keys.rs`.
  Stated as `Vec + MessageField == 4 * size_of::<usize>()`, width
  independent. This is the pin that matters there.

Budgets, asserted with `<=`:

- `SenderKeyState <= 224` (64-bit) and `<= 204` (32-bit),
  `SenderKeyStateStructure <= 48` and `<= 28`, same file. The total floats
  with the protobuf runtime layout, so only the direction is pinned.
- `Slot<u32, Arc<str>> <= 56`, `Slot<SenderMessageId, ()> <= 128` in
  `src/portable_cache.rs`, 64-bit only. The win is the flattened layout
  reusing tail padding. Smaller stays fine.
- `UreqHttpClient <= Agent + 24` in `http_clients/ureq-client/src/lib.rs`,
  64-bit only. The `Agent` half moves with each ureq release, so the assert
  floats with it. The companion `< Agent + Option<HttpResourceReport>` is
  the actual contract.
- `DeviceListRecord <= 64` in `wacore/src/store/traits.rs`. One record lives
  per known contact, so this is a per-contact budget.
- `RuntimeCacheConfig <= 136` in `src/cache_config.rs`, with the companion
  ratio check against `CacheConfig`.
- `UsyncProtocolResult <= 96` in `wacore/src/iq/usync/query.rs`, send
  futures `<= 192` in `src/send/mod.rs`, dispatch claim `<= 56` in
  `src/message/tests.rs`, group snapshot `<= 92` per participant in
  `wacore/src/client/context.rs`, sender-key map `<= 36` per device in
  `src/sender_key_device_cache.rs`. Pure budgets, already bounds.

## Rebaseline procedure

1. Run just the failing test and read the actual size from the failure
   output. Confirm nothing else in that test failed.
2. Find what moved. `git log` on the struct, `cargo tree` for the dependency
   that owns an embedded field, `rustc --version` for a toolchain repack.
   The failure comment names the usual suspect for that assert.
3. Check the compositional assert first. If the width independent property
   still holds, the design did not regress. Only the number drifted.
4. Audit the delta field by field. A new field with a reason is fine.
   Unexplained growth, or growth from a dependency you did not intend to
   take, is a real finding. Fix that instead.
5. Update the bound and the comment next to it. Say what moved and why the
   new number is right. Never bump a number just to turn CI green.
6. Run the layout tests listed below and keep the whole diff to tests plus
   this file. No production code changes.

Layout tests to run, one filter per command so a failure names its assert:

```bash
cargo test -p wacore-libsignal --lib sender_key_state_layout_dropped_the_protobuf_copies
cargo test -p wacore-binary --lib the_hint_tape_stays_five_bytes_wide
cargo test -p wacore --lib a_device_entry_is_eight_bytes
cargo test -p wacore --lib a_device_list_record_fits_sixty_four_bytes
cargo test -p wacore --lib sparse_result_layout_stays_bounded
cargo test -p wacore --lib retained_bytes_per_participant_stay_bounded
cargo test -p whatsapp-rust --lib flattened_slot_reuses_entry_tail_padding
cargo test -p whatsapp-rust --lib runtime_config_is_compact
cargo test -p whatsapp-rust --lib queued_chat_message_keeps_two_handles
cargo test -p whatsapp-rust --lib send_futures_stay_small
cargo test -p whatsapp-rust --lib pdo_alias_claim_stays_small
cargo test -p whatsapp-rust --lib retained_bytes_per_device_stay_bounded
cargo test -p whatsapp-rust-ureq-http-client --lib provenance_reconstructs_the_stored_report
```
