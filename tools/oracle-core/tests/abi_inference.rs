//! Inferring parameter roles from bytecode.
//!
//! The point of this analysis is that it needs nothing but the module: no name
//! section, no glue, no debug info. These tests run it against the captures
//! where every one of those is absent.

use oracle_core::abi::{self, Role};

mod common;

/// The capture every index and address below was read out of.
///
/// Named once because a capture bump invalidates all of them at the same time:
/// the module behind the name stays the same program, but the binary renumbers
/// its functions and moves its data, so the reads still succeed and answer
/// about different code. See "Following a capture forward" in `README.md`.
const VOIP: &str = "JgwtTQVeWPm";

/// Serialises the tests that set the body limit.
///
/// `abi::set_body_limit` is process-global and cargo runs a binary's tests on
/// several threads, so two of them setting it race: one asks for 30,000
/// instructions to read a dispatcher, the other asks for 16 to check
/// truncation, and whichever lands second decides what the first one sees. The
/// symptom is `a_branch_table_lists_its_targets` reporting no `br_table` at all
/// — a body cut to sixteen instructions has none — and it fails perhaps one run
/// in ten, which is exactly the kind of flake that gets blamed on the capture.
static BODY_LIMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn module(id: &str) -> Option<Vec<u8>> {
    common::capture(id).unwrap_or_else(|error| panic!("loading {id}: {error:#}"))
}

macro_rules! module_or_skip {
    ($id:expr) => {
        match module($id) {
            Some(bytes) => bytes,
            None => {
                eprintln!("skipping: {} unavailable (set WA_WASM_DIR)", $id);
                return;
            }
        }
    };
}

/// A minified module gives up nothing but its code, and that is enough.
///
/// mozjpeg's exports are single letters; `B` is its allocator, and its one
/// argument is a size. The analysis has to say so from the code alone — it
/// bounds-checks the request and does arithmetic on it, and is never
/// dereferenced.
#[test]
fn a_minified_module_still_yields_evidence() {
    let bytes = module_or_skip!("php8T1oSIZM");
    let inferred = abi::infer(&bytes, |name| name == "B").expect("infer");

    let alloc = inferred.first().expect("B should be exported");
    assert_eq!(alloc.params.len(), 1);
    assert!(
        alloc.scanned > 10,
        "the allocator body should have been scanned, saw {}",
        alloc.scanned
    );
    assert_eq!(
        alloc.params[0].role(),
        Role::Length,
        "an allocator's argument is a size: {}",
        alloc.params[0].evidence()
    );
}

/// The entry point nobody could name: six integers, and every one of them has
/// to come back with evidence behind it.
///
/// This is the classification that the mozjpeg call in `differential.rs` was
/// built on, so a regression here is a regression in the only handle anyone has
/// on that module.
#[test]
fn mozjpeg_entry_point_arguments_are_classified() {
    let bytes = module_or_skip!("php8T1oSIZM");
    let inferred = abi::infer(&bytes, |name| name == "z").expect("infer");
    let entry = inferred.first().expect("z should be exported");

    assert_eq!(entry.params.len(), 6);
    assert!(
        entry.scanned > 100,
        "a real entry point should have a substantial body, saw {}",
        entry.scanned
    );
    for param in &entry.params {
        assert_ne!(
            param.role(),
            Role::Unknown,
            "arg{} came back unclassified: {}",
            param.index,
            param.evidence()
        );
    }

    // The width and the height are written straight into the compress struct
    // and never touched again, which is what makes them readable at all.
    assert_eq!(entry.params[2].stored_into, 1, "arg2 is stored as a value");
    assert_eq!(entry.params[3].stored_into, 1, "arg3 is stored as a value");
}

/// A wasm backtrace names its frames by index, so the index has to be a way in.
///
/// Function 53 of the mozjpeg module is the frame every failed call landed on,
/// and this is what made it readable: three instructions, and the constant they
/// carry is the panic message.
#[test]
fn a_function_can_be_read_by_index_and_quotes_its_strings() {
    let bytes = module_or_skip!("php8T1oSIZM");
    let _limit = BODY_LIMIT.lock().unwrap_or_else(|e| e.into_inner());
    abi::set_body_limit(16);

    let panicker = abi::infer_index(&bytes, 53)
        .expect("infer")
        .expect("function 53 exists");

    assert_eq!(panicker.index, 53);
    let listing = panicker.body.join("\n");
    assert!(
        listing.contains("called `Option::unwrap()` on a `None` value"),
        "the constant addresses a panic message, listing was:\n{listing}"
    );
    // The `i32.const 43` that follows is the slice's length. Without it the
    // quote runs on into the next message, since neither Rust nor C++ terminates
    // these strings.
    assert!(
        !listing.contains("valuepanicked"),
        "the quote should stop at the length the next instruction gives:\n{listing}"
    );

    // An import has no body, but naming it is the point: a trap in `a.d` means
    // something different from a trap in guest code.
    let import = abi::infer_index(&bytes, 0)
        .expect("infer")
        .expect("function 0 is an import");
    assert!(
        import.name.contains('.'),
        "an import should be named module.field, got {}",
        import.name
    );
}

/// `invoke_*` trampolines have to be found by shape, not by name.
///
/// Stubbing one is not neutral — the call it should have dispatched silently
/// does not happen and the guest reads back a result nobody produced. On a
/// minified module the name is gone, so the detection is validated against a
/// module that still has its names: every declared trampoline must be found,
/// and nothing else may be.
#[test]
fn invoke_trampolines_are_found_by_shape() {
    let bytes = module_or_skip!("COs9e0Kj0ic");
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &bytes).expect("load module");

    let by_shape = abi::find_invoke_imports(&bytes).expect("scan");
    let found: Vec<String> = module
        .imports()
        .filter(|import| matches!(import.ty(), wasmtime::ExternType::Func(_)))
        .enumerate()
        .filter(|(index, _)| by_shape.contains_key(&(*index as u32)))
        .map(|(_, import)| import.name().to_owned())
        .collect();

    let declared: Vec<String> = module
        .imports()
        .filter(|import| import.name().starts_with("invoke_"))
        .map(|import| import.name().to_owned())
        .collect();
    assert!(!declared.is_empty(), "this module should declare invokes");

    for name in &declared {
        assert!(found.contains(name), "{name} was missed by the shape scan");
    }
    for name in &found {
        assert!(
            name.starts_with("invoke_"),
            "{name} is not a trampoline but was detected as one"
        );
    }

    // Every trampoline flags through the same `__THREW__` word; two addresses
    // would mean the pattern matched something else as well.
    let addresses: std::collections::BTreeSet<u32> = by_shape.values().copied().collect();
    assert_eq!(
        addresses.len(),
        1,
        "expected one threw flag, got {addresses:?}"
    );
}

/// The minified module's own trampolines, which have no names left at all.
#[test]
fn a_minified_module_yields_its_trampolines() {
    let bytes = module_or_skip!("php8T1oSIZM");
    let by_shape = abi::find_invoke_imports(&bytes).expect("scan");

    assert!(
        !by_shape.is_empty(),
        "mozjpeg dispatches through the table; without these its \
         jpeg_start_compress is never called"
    );
    let addresses: std::collections::BTreeSet<u32> = by_shape.values().copied().collect();
    assert_eq!(
        addresses.len(),
        1,
        "expected one threw flag, got {addresses:?}"
    );
}
/// A trampoline is recognised as one, which changes what a caller must do: the
/// arguments that matter belong to the callee, and the first argument has to be
/// an object whose function pointer is already filled in.
#[test]
fn voprf_blind_is_a_trampoline() {
    let bytes = module_or_skip!("COs9e0Kj0ic");
    let inferred = abi::infer(&bytes, |name| name == "blind").expect("infer");
    let blind = inferred.first().expect("blind should be exported");

    assert!(
        blind.is_trampoline(),
        "blind should be recognised as a trampoline: {}",
        blind.describe()
    );
    assert!(
        blind.body.iter().any(|line| line.contains("call_indirect")),
        "a trampoline calls through a pointer: {:?}",
        blind.body
    );
    assert!(
        blind.body.iter().any(|line| line.contains("i32.load")),
        "and loads that pointer from its first argument: {:?}",
        blind.body
    );
}

/// Following the trampoline: a table slot resolves to the function that really
/// takes the arguments.
#[test]
fn a_table_slot_resolves_to_a_function() {
    let bytes = module_or_skip!("COs9e0Kj0ic");

    // Slot 2 is what a Ristretto curve object carries in its vtable; see
    // `examples/voprf_flow.rs`, which reads it out of live memory.
    let target = abi::infer_table_slot(&bytes, 2)
        .expect("infer")
        .expect("slot 2 should hold a function");

    assert!(
        target.scanned > 0,
        "the resolved function should have a body"
    );
    assert!(
        !target.params.is_empty(),
        "and should take the arguments the trampoline forwards"
    );
}

/// An out-of-range slot is reported as empty rather than guessed at.
#[test]
fn an_unpopulated_slot_reports_nothing() {
    let bytes = module_or_skip!("COs9e0Kj0ic");
    assert!(
        abi::infer_table_slot(&bytes, 999_999)
            .expect("infer")
            .is_none()
    );
}

/// The analysis works on a module built from a completely different toolchain,
/// which is what makes it useful for captures that do not exist yet.
#[test]
fn inference_works_across_toolchains() {
    // Rust/WASI, name section intact — the opposite of the minified C++ ones.
    let bytes = module_or_skip!("rogm88TRRiw");
    let inferred = abi::infer(&bytes, |name| name == "convertFixed32BitToFloat").expect("infer");

    let convert = inferred.first().expect("function should be exported");
    assert_eq!(convert.params.len(), 2);
    // It is pure arithmetic on two scalars: nothing should be read as a buffer.
    for param in &convert.params {
        assert!(
            !matches!(param.role(), Role::Pointer { .. }),
            "arg{} misclassified as a pointer: {}",
            param.index,
            param.evidence()
        );
    }
}

/// A module with threads keeps its strings in *passive* data segments.
///
/// Shared memory rules out active segments — the memory is initialised once
/// rather than per instance — so emscripten emits everything passive and copies
/// it in with `memory.init`. The address of every string then lives in the code
/// instead of the data section, and without following that copy not one string
/// in the VoIP module can be read.
#[test]
fn passive_segments_are_placed_where_memory_init_puts_them() {
    let bytes = module_or_skip!(VOIP);

    // A message this module is known to carry, and the code that reaches it.
    let found = abi::find_string_refs(&bytes, "record_incoming_msg: no active call").expect("scan");
    let entry = found.first().expect("the message should be in the module");

    assert!(
        entry.text.contains("no active call"),
        "the text at the address should be the message, got {:?}",
        entry.text
    );
    assert!(
        !entry.referenced_by.is_empty(),
        "a log message is loaded by whatever logs it"
    );
}

/// Naming a function means reading the constants its logging call is handed,
/// and that only works if an address resolves through the passive segments.
///
/// Deriving the mapping by hand — segment base plus offset, straight off the
/// data section — silently skips the `memory.init` placement above and returns
/// text from the wrong address: not an error, just a plausible-looking string
/// that belongs to some other file. Tracking down a `70008` cost a detour that
/// way, so the module's own reader is the only thing that should do this.
#[test]
fn an_address_resolves_to_the_text_the_module_actually_places_there() {
    let bytes = module_or_skip!(VOIP);

    let found = abi::find_string_refs(&bytes, "record_incoming_msg: no active call").expect("scan");
    let entry = found.first().expect("the message should be in the module");

    let text = abi::static_string(&bytes, entry.address)
        .expect("resolve")
        .expect("the address the scan reported must hold text");

    assert!(
        text.starts_with("record_incoming_msg: no active call"),
        "resolving the scan's own address should return the scan's own string, got {text:?}"
    );

    // One byte in is still inside the string, and must not fall back to some
    // other segment — the failure mode being pinned is a plausible wrong answer,
    // not an empty one.
    let shifted = abi::static_string(&bytes, entry.address + 1)
        .expect("resolve")
        .expect("one byte into a string is still text");
    assert!(
        text.ends_with(&shifted),
        "reading from {} should be the tail of reading from {}, got {shifted:?}",
        entry.address + 1,
        entry.address
    );
}

/// Some strings are reached only through a table of pointers.
///
/// An enum's names are indexed, never named, so no instruction mentions them.
/// Reporting such a string as unreferenced would read as "dead code" when it is
/// simply one indirection away.
#[test]
fn a_string_reached_through_a_table_reports_its_pointer() {
    let bytes = module_or_skip!(VOIP);

    let found = abi::find_string_refs(&bytes, "EVENT: Call missed by the user").expect("scan");
    let entry = found
        .first()
        .expect("the event name should be in the module");

    assert!(
        entry.referenced_by.is_empty(),
        "this one is indexed, not named"
    );
    assert!(
        !entry.pointer_slots.is_empty(),
        "and the pointer to it is what makes it findable"
    );

    // The table base is what the emitting code actually holds.
    let slot = entry.pointer_slots[0];
    let refs = abi::find_address_refs(&bytes, slot, 512).expect("scan");
    assert!(
        !refs.is_empty(),
        "nothing within 512 bytes before {slot:#x} is loaded as a constant"
    );
    assert!(
        refs.iter().all(|entry| entry.offset % 4 == 0),
        "a pointer table is indexed by whole words"
    );
}

/// Walking back up the call graph, which a stripped module does not provide.
#[test]
fn callers_are_found_by_scanning_call_sites() {
    let bytes = module_or_skip!(VOIP);

    // The proxy import every worker thread goes through to reach the main one.
    let callers = abi::find_callers(&bytes, 76).expect("scan");
    assert!(
        !callers.is_empty(),
        "emscripten_receive_on_main_thread_js is called by the queue drainer"
    );

    // An index past the end of the module is called by nobody, rather than
    // producing a spurious hit.
    assert!(
        abi::find_callers(&bytes, 9_999_999)
            .expect("scan")
            .is_empty()
    );
}

/// A function nothing calls directly is a callback, not dead code.
///
/// The VoIP engine's outbound signaling goes through one: it is reached only
/// once something stores its table slot, and the slot is the difference between
/// "unreachable" and "runs when this vtable entry is filled in".
#[test]
fn a_callback_reports_the_slot_that_reaches_it() {
    let bytes = module_or_skip!(VOIP);

    let sender = abi::find_callers(&bytes, 21).expect("scan");
    let sender = *sender
        .first()
        .expect("something calls sendSignalingXMPP_js_sync");

    assert!(
        abi::find_callers(&bytes, sender).expect("scan").is_empty(),
        "the sender is a callback: nothing calls it directly"
    );

    let abi = abi::infer_index(&bytes, sender)
        .expect("infer")
        .expect("the function exists");
    assert!(
        !abi.table_slots.is_empty(),
        "so it has to be reachable through the table"
    );
}

/// A callback reaches the guest as a bare table index.
///
/// Nothing calls it, nothing points at it as an address — the only trace is an
/// `i32.const` somewhere. That is how the VoIP engine's outbound signaling is
/// wired: `sendSignalingXMPP_js_sync` is called only by the function in table
/// slot 436, which is itself dispatched by slot 437, which something else
/// loads as a constant. Neither `find_callers` nor `find_address_refs` can see
/// any of those hops.
#[test]
fn a_table_slot_is_traced_through_the_constant_that_names_it() {
    let bytes = module_or_skip!(VOIP);

    // The sender: reached only through the table.
    let sender = *abi::find_callers(&bytes, 21)
        .expect("scan")
        .first()
        .expect("something calls sendSignalingXMPP_js_sync");
    let slots = abi::infer_index(&bytes, sender)
        .expect("infer")
        .expect("the sender exists")
        .table_slots;
    let slot = *slots.first().expect("the sender sits in the table");

    // And the constant that names it leads to whoever dispatches it.
    let users = abi::find_constant_users(&bytes, slot as i32).expect("scan");
    assert!(
        !users.is_empty(),
        "slot {slot} must be named by a constant somewhere, or nothing could \
         ever dispatch it"
    );

    // The count is what makes a hit judgeable: a small constant appears in
    // unrelated arithmetic, and one use in a large function is usually noise.
    assert!(
        users.iter().all(|(_, uses)| *uses > 0),
        "a reported user must have used it at least once"
    );
}

/// A `br_table` without its targets is unreadable.
///
/// A switch renders as one opaque jump, and the only way left to match a case
/// to its body is the order the bodies appear in the listing — which is not the
/// order of the labels. That guesswork produced a wrong reading of the VoIP
/// engine's signaling dispatcher, and the labels are what corrected it.
#[test]
fn a_branch_table_lists_its_targets() {
    let bytes = module_or_skip!(VOIP);
    let _limit = BODY_LIMIT.lock().unwrap_or_else(|e| e.into_inner());
    abi::set_body_limit(30_000);

    // A dispatcher the engine reaches through its function table (slot 9788),
    // carrying twelve switches — the widest of them 246 cases.
    //
    // Carried forward from the previous capture, where it was func 11995, by
    // `unwasm`'s fingerprint: the hash covers a body's shape and signature and
    // deliberately drops what a rebuild changes anyway — constant values, call
    // targets, global indices. It is unique on both sides, and the br_table
    // width agrees at 246, which is a second and independent fact. That is the
    // route to use for the next capture too: this index cannot be guessed, and
    // the function it names is not the one that held the number before.
    let dispatcher = abi::infer_index(&bytes, 13_364)
        .expect("infer")
        .expect("the dispatcher exists");

    let tables: Vec<&String> = dispatcher
        .body
        .iter()
        .filter(|line| line.starts_with("br_table"))
        .collect();
    assert!(
        !tables.is_empty(),
        "the dispatcher switches on its argument"
    );
    for table in tables {
        assert!(
            table.contains("default"),
            "every br_table has a default target: {table}"
        );
        assert!(
            table.contains('[') && table.contains(']'),
            "and its targets should be listed: {table}"
        );
    }
}
