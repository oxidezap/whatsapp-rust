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
