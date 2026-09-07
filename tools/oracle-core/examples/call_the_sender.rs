//! Call the signaling trampoline directly, and see what reaches the host.
//!
//! Function **#855** is the only caller of `sendSignalingXMPP_js_sync`, and
//! `oracle callers` reports that nothing calls it *directly* — it is reached
//! through the table, at slot **464** (`table_slot_of`). This calls the slot by
//! hand, which answers what no amount of watching can: whether #855 does what
//! its body says, independently of who reaches it.
//!
//! It also produced the correction the rest of this investigation needed. The
//! import's recording stub reports **zero** calls here while the *counter*
//! reports one — `all_calls_to` reads a list capped at `MAX_TRACE` (8192) and
//! the run makes ~39 million host calls. Every earlier "nothing was sent" was
//! that artefact, including the reading that #855 never executes.
//!
//! `oracle abi S_ivh1PriOA --index 855` reads out in full:
//!
//! ```wat
//! func[855](arg0: i32)
//!   if arg0 != 0 {
//!     call 21 ( [arg0+0], [arg0+4], [arg0+8], [arg0+12] )   ;; the import
//!     call 613([arg0+0]); call 613([arg0+4]); call 613([arg0+8]); call 613(arg0)
//!   }
//! ```
//!
//! So it is an unpacking trampoline: one pointer to a four-word block, forwarded
//! to the import, then three of the words and the block itself are freed. That
//! is emscripten's proxy shape — a worker allocates the block, queues
//! `(slot, block)`, and the main thread runs this.
//!
//! This builds such a block by hand and calls the slot. The counter goes to one
//! and the marker on #855's entry fires, so the trampoline is proven to be the
//! delivery path — and `outbound_setup_matrix` shows the engine reaching it on
//! its own during an ordinary origination.
//!
//! ```sh
//! cargo run --release --example call_the_sender
//! cargo run --release --example call_the_sender -- JgwtTQVeWPm
//! ```
use anyhow::{Result, bail};
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wasmtime::Val;

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
const SELF_LID: &str = "99887766554433:0@lid";

/// The slot `table_slot_of` reports for #855 on this capture. Passed rather
/// than assumed, so a capture whose numbering moved fails loudly.
const SENDER_SLOT: u32 = 464;

/// Three of the four words are freed by the trampoline, so they have to be
/// `malloc`ed rather than pointed at anything else — freeing a pointer the
/// allocator did not hand out corrupts the heap, and the trap would land
/// somewhere unrelated. The fourth is left as a plain integer, which #855 does
/// not free.
fn nul_terminated(r: &mut Runtime, text: &str) -> Result<u32> {
    let bytes = text.as_bytes();
    let ptr = r.malloc(bytes.len() as u32 + 1)?;
    r.write_bytes_at(ptr, bytes)?;
    r.write_bytes_at(ptr + bytes.len() as u32, &[0])?;
    Ok(ptr)
}

fn main() -> Result<()> {
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "S_ivh1PriOA".into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    println!("{}\n", entry.id);

    let mut r = Runtime::instantiate(&bytes)?;
    r.set_thread_policy(ThreadPolicy::Spawn);
    r.run_ctors()?;
    r.attach_log_ring(1 << 20)?;
    // So an instrumented copy can say whether #855 itself ran. Without this the
    // mirror records nothing and a silent marker is indistinguishable from a
    // marker that was never watched.
    r.shared().watch_markers("env::on_call_event_js_sync");

    // The stack has to be up: `malloc` and the trampoline's `free` both need a
    // running allocator, and the import's stub is only interesting once the
    // engine is in the state a real send would happen in.
    let init = r.call_embind(
        "initVoipStack",
        &[
            Value::Str(SELF.into()),
            Value::Str(SELF_DEVICE.into()),
            Value::Str(SELF_LID.into()),
        ],
    );
    r.refuel();
    println!("initVoipStack -> {init:?}");

    if !r.table_entry_exists(SENDER_SLOT) {
        bail!(
            "table slot {SENDER_SLOT} is empty on this capture — re-derive it with table_slot_of"
        );
    }

    // A four-word block shaped like what the trampoline forwards. The import's
    // signature is `(i32, i32, i32, i32)` and three of the words are freed, so
    // three are heap strings and the fourth is a plain value.
    let stanza = nul_terminated(&mut r, "<call to=\"probe\"><offer/></call>")?;
    let call_id = nul_terminated(&mut r, "0011223344556677")?;
    let peer = nul_terminated(&mut r, "11223344556677@lid")?;
    let block = r.malloc(16)?;
    for (i, word) in [stanza, call_id, peer, 0xABCD].iter().enumerate() {
        r.write_bytes_at(block + 4 * i as u32, &word.to_le_bytes())?;
    }
    println!("block at {block:#x}: [{stanza:#x}, {call_id:#x}, {peer:#x}, 0xabcd]\n");

    // `all_calls_to` only sees imports that got a recording stub, so a zero
    // from it means nothing until this says the stub is there.
    println!(
        "sender has a recording stub: {}",
        r.stubbed_imports()
            .contains("env::sendSignalingXMPP_js_sync")
    );

    let before = r.all_calls_to("env::sendSignalingXMPP_js_sync").len();
    let outcome = r.call_table(SENDER_SLOT, &[Val::I32(block as i32)]);
    r.refuel();
    let calls = r.all_calls_to("env::sendSignalingXMPP_js_sync");

    println!(
        "call_table({SENDER_SLOT}) -> {}",
        match &outcome {
            Ok(v) => format!("{v:?}"),
            Err(e) => format!("error: {e:#}"),
        }
    );
    println!(
        "sendSignalingXMPP_js_sync: {} call(s) before, {} after",
        before,
        calls.len()
    );
    for call in calls.iter().skip(before) {
        println!("  args {:?}", call.args);
    }
    let markers: Vec<i32> = r.shared().markers().iter().map(|(id, _)| *id).collect();
    println!("markers so far: {markers:?}");
    // A second counter over the same event, because `all_calls_to` answering
    // zero while the trampoline demonstrably ran is either a real "it did not
    // call" or a hole in that accessor, and the two need separating.
    // `all_calls_to` reads the recorded-call *list*, which stops growing at
    // `MAX_TRACE` (8192) while the counters keep going. On a run where the
    // engine has made tens of thousands of host calls, a zero from it means
    // "not in the first 8192", not "never happened" — which is a trap this
    // investigation fell into twice.
    let counts = r.shared().hot_calls();
    let sender = counts
        .iter()
        .find(|(symbol, _)| symbol == "env::sendSignalingXMPP_js_sync")
        .map(|(_, n)| *n)
        .unwrap_or(0);
    println!(
        "counter says sendSignalingXMPP_js_sync: {sender} call(s)  (total host calls: {})",
        r.shared().total_calls()
    );

    // The null guard, which is the other half of reading the body correctly: a
    // zero pointer must return without touching the import. Checking it costs
    // one call and turns "the trampoline forwards" into "the trampoline
    // forwards exactly what its body says".
    let mark = r.all_calls_to("env::sendSignalingXMPP_js_sync").len();
    let null = r.call_table(SENDER_SLOT, &[Val::I32(0)]);
    r.refuel();
    println!(
        "\nnull block -> {} , {} further call(s) to the import",
        match &null {
            Ok(v) => format!("{v:?}"),
            Err(e) => format!("error: {e:#}"),
        },
        r.all_calls_to("env::sendSignalingXMPP_js_sync").len() - mark
    );

    Ok(())
}
