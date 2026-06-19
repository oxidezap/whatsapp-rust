//
// Copyright 2020 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

pub const MAX_FORWARD_JUMPS: usize = 25_000;
pub const MAX_MESSAGE_KEYS: usize = 2000;
pub const MAX_RECEIVER_CHAINS: usize = 5;
pub const ARCHIVED_STATES_MAX_LENGTH: usize = 40;
pub const MAX_SENDER_KEY_STATES: usize = 5;

/// Threshold for amortized message key eviction.
/// Eviction only triggers when buffer exceeds MAX_MESSAGE_KEYS + PRUNE_THRESHOLD,
/// so the prune runs once every PRUNE_THRESHOLD inserts instead of on every insert.
pub const MESSAGE_KEY_PRUNE_THRESHOLD: usize = 50;

/// Drop the `excess` oldest entries from a skipped-message-key buffer in O(excess).
///
/// These buffers are searched by counter/iteration, never by position (see
/// `SessionState::get_message_keys` and `SenderKeyState::remove_sender_message_key`),
/// so survivors may be reordered: swapping the tail into the evicted front and
/// truncating beats `Vec::drain(..excess)`, which shifts the ~MAX_MESSAGE_KEYS
/// survivors down. `excess` is always far smaller than the surviving count here.
pub(crate) fn evict_oldest_message_keys<T>(keys: &mut Vec<T>, excess: usize) {
    let keep = keys.len() - excess;
    debug_assert!(
        excess <= keep,
        "eviction must not exceed the surviving count"
    );
    for i in 0..excess {
        keys.swap(i, keep + i);
    }
    keys.truncate(keep);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The helper must retain exactly the `drain(..excess)` survivor set — the
    /// oldest `excess` entries gone, the rest kept (order is allowed to differ).
    #[test]
    fn evict_oldest_message_keys_keeps_the_drain_survivor_set() {
        let total = MAX_MESSAGE_KEYS + MESSAGE_KEY_PRUNE_THRESHOLD + 1;
        let excess = total - MAX_MESSAGE_KEYS;
        // Distinct values standing in for keys in arrival order.
        let mut keys: Vec<u32> = (0..total as u32).collect();

        evict_oldest_message_keys(&mut keys, excess);

        assert_eq!(keys.len(), MAX_MESSAGE_KEYS);
        let retained: HashSet<u32> = keys.into_iter().collect();
        let expected: HashSet<u32> = (excess as u32..total as u32).collect();
        assert_eq!(retained, expected);
    }
}
