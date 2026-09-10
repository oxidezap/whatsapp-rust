//! Static map of the J engine's transport-egress and capture imports.
//!
//! Fast (no instantiation): pins the function indices of the relay-send and
//! capture imports plus the table-slot chain from the video/audio senders
//! down to `env::call_sendto`. The follow-up established-call harness (relay
//! emulation) starts from slot 7472 instead of re-deriving it.
//!
//! `cargo test -p oracle-core --test video_sender_static -- --nocapture`

mod common;

use anyhow::Result;
use oracle_core::abi;
use sha2::{Digest, Sha256};

const CAPTURE: &str = "JgwtTQVeWPm";
const CAPTURE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

#[test]
fn transport_egress_chain() -> Result<()> {
    let Some(bytes) = common::capture(CAPTURE)? else {
        eprintln!("skipping: {CAPTURE} unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    assert_eq!(hex::encode(Sha256::digest(&bytes)), CAPTURE_SHA);

    // Import indices of the transport and capture host functions.
    let mut import_index = std::collections::HashMap::new();
    {
        use wasmparser::{Parser, Payload};
        for payload in Parser::new(0).parse_all(&bytes) {
            let payload = payload.map_err(anyhow::Error::msg)?;
            if let Payload::ImportSection(reader) = payload {
                let mut func_index = 0u32;
                for import in reader.into_imports() {
                    let import = import.map_err(anyhow::Error::msg)?;
                    if matches!(
                        import.ty,
                        wasmparser::TypeRef::Func(_) | wasmparser::TypeRef::FuncExact(_)
                    ) {
                        import_index
                            .insert(format!("{}::{}", import.module, import.name), func_index);
                        func_index += 1;
                    }
                }
            }
        }
    }
    // NOTE: non-function imports do not advance the function index space, so
    // count only Func imports (as above).
    let sendto = *import_index
        .get("env::call_sendto")
        .context("call_sendto import")?;
    assert_eq!(abi::find_callers(&bytes, sendto)?, vec![10078]);
    assert_eq!(abi::table_slots_of(&bytes, 10078)?, vec![7472]);
    // Two paths feed the egress slot: keep both pinned for the follow-up.
    for (func, slot) in [(10089u32, 7490u32), (12912u32, 9306u32)] {
        assert_eq!(abi::table_slots_of(&bytes, func)?, vec![slot]);
        let users: Vec<u32> = abi::find_constant_users(&bytes, slot as i32)?
            .into_iter()
            .map(|(f, _)| f)
            .collect();
        assert!(!users.is_empty(), "slot {slot} must have a user");
    }
    // Capture-start import and its table slot (invoked indirectly).
    let cap = *import_index
        .get("env::call_start_video_capture_js_sync")
        .context("capture import")?;
    assert_eq!(abi::table_slots_of(&bytes, cap)?, vec![9351]);
    eprintln!("egress chain pinned: call_sendto={sendto} -> 10078 @7472, capture={cap} @9351");
    Ok(())
}
