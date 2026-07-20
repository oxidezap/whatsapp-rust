//
// Copyright 2020 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

/// Max chain steps derived in a single decrypt for a normal (peer) session or
/// a group sender-key. Matches WA Web's `signalFutureMessagesMax` (2000): a
/// message whose counter is more than this far ahead is rejected (→
/// retry-receipt path) instead of forcing thousands of KDF derivations, which
/// would be a per-message CPU amplification / DoS surface.
pub const MAX_FORWARD_JUMPS: usize = 2_000;
/// Wider bound for a pairwise session with our OWN other devices. Multi-device
/// app-sync legitimately jumps far ahead and the peer is trusted, so the DoS
/// concern does not apply; WA Web uses 2000 uniformly but we keep a wider (yet
/// still bounded — previously this path was effectively unbounded) ceiling to
/// avoid regressing self-sync decryption.
pub const MAX_FORWARD_JUMPS_SELF: usize = 25_000;
pub const MAX_MESSAGE_KEYS: usize = 2000;
pub const MAX_RECEIVER_CHAINS: usize = 5;
pub const ARCHIVED_STATES_MAX_LENGTH: usize = 40;
pub const MAX_SENDER_KEY_STATES: usize = 5;

/// Threshold for amortized message key eviction.
/// Eviction only triggers when buffer exceeds MAX_MESSAGE_KEYS + PRUNE_THRESHOLD,
/// reducing O(n) drain() calls from every insert to once every PRUNE_THRESHOLD inserts.
pub const MESSAGE_KEY_PRUNE_THRESHOLD: usize = 50;

/// Sender-chain counters leased per durable reservation (see
/// `SessionRecord::reserve_sender_chain_counters`). Message keys and IVs are
/// derived deterministically from the counter, so an outbound counter must
/// never repeat across a crash; instead of persisting every advance before it
/// hits the wire, the record durably reserves this many counters ahead and a
/// reloaded snapshot fast-forwards past them. Bounds both the sync-flush
/// amortization (one per this many sends) and the worst-case counter gap a
/// receiver sees after a crash — keep it well under MAX_FORWARD_JUMPS.
pub const SENDER_CHAIN_RESERVATION_BATCH: u32 = 64;

/// Upper bound for the reservation fast-forward on load. A legitimate lease
/// gap is < SENDER_CHAIN_RESERVATION_BATCH; anything past this ceiling means
/// a corrupt record, and refusing it keeps a bogus reserved index from
/// turning the load into an unbounded KDF loop.
pub const MAX_RESERVATION_FAST_FORWARD: u32 = MAX_FORWARD_JUMPS as u32;
