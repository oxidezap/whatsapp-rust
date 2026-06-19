#![no_main]

//! Fuzz the WABinary decoder entry point.
//!
//! `unmarshal_ref` is the largest hostile-input surface in the crate: it parses
//! attacker-controlled bytes straight off the wire into a `NodeRef` tree. The
//! target feeds arbitrary bytes in and discards the `Result` — a malformed input
//! must return `Err`, never panic, overflow the stack, or abort.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = wacore_binary::marshal::unmarshal_ref(data);
});
