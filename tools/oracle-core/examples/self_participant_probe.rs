//! Why does `make_and_cache_offer` say the group has no self participant?
//!
//! It fails at `offer.cc:463`, where it requires
//! `wa_call_group_get_self_participant` — which returns `group->[592]` — to be
//! non-null. `getCallInfo` says a participant *is* marked `is_self`, so the two
//! disagree, and separating them means reading those words rather than
//! inferring them.
//!
//! That was impossible while the call context lived in an unexported global.
//! `cargo xt oracle export-globals` fixes that:
//!
//! ```sh
//! cargo xt oracle export-globals caps/JgwtTQVeWPm.wasm .cache/patched/JgwtTQVeWPm.wasm 15
//! WA_WASM_DIR=.cache/patched cargo run --release --example self_participant_probe
//! ```
//!
//! Reads, all from the same context pointer:
//!   * `call + 48`      — the byte `reset_group_and_self_participant` returns on
//!   * `call + 659164`  — the group
//!   * `group + 592`    — the self participant the offer path demands
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

/// Fictitious, like every identity in this repository.
const SELF: &str = "99887766554433@lid";
const SELF_DEVICE: &str = "15550002222@s.whatsapp.net";
const SELF_LID: &str = "99887766554433@lid";
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";

/// Offsets established from the disassembly; see `agent_docs/voip_oracle_status.md`.
const GROUP_IN_CALL: u32 = 659_164;
const SELF_IN_GROUP: u32 = 592;

/// Dumps every exported global, so the context can be found by which one moves.
///
/// `global 10` was the obvious guess — `startVoipCall` opens with
/// `global.get 10` — and it is wrong: it reads 0x18 and never changes, so it is
/// a constant, not a pointer. Finding the real one is a measurement, not a
/// reading.
fn dump_globals(runtime: &mut Runtime, when: &str) {
    let mut seen = Vec::new();
    for index in 0..15 {
        if let Ok(value) = runtime.global_i32(&format!("__global_{index}")) {
            seen.push(format!("{index}={value:#x}"));
        }
    }
    println!("{when}: {}", seen.join(" "));
}

/// Finds the call context by scanning, since it is in neither a global nor an
/// obvious static address.
///
/// The call id is a 16-character string the engine stores inside the call
/// object, so every hit is a candidate `base + k`. A candidate is accepted when
/// `base + 659164` holds something that looks like a heap pointer — that word is
/// the group, and the offer path reads the self participant out of it.
fn find_context(runtime: &mut Runtime, call_id: &str) -> Vec<(u32, u32, u32, u32, u32)> {
    const CHUNK: u32 = 1 << 20;
    const LIMIT: u32 = 64 << 20;

    let needle = call_id.as_bytes();
    let mut hits = Vec::new();
    let mut at = 0u32;
    let mut scanned = 0u64;
    while at < LIMIT {
        let Ok(block) = runtime.read(at, CHUNK) else {
            break;
        };
        scanned += block.len() as u64;
        for (offset, window) in block.windows(needle.len()).enumerate() {
            if window == needle {
                hits.push(at + offset as u32);
            }
        }
        at += CHUNK - needle.len() as u32;
    }
    println!(
        "scanned {scanned} bytes; call id found at {} address(es)",
        hits.len()
    );

    // The call id sits at some unknown offset inside the object, so walk back a
    // little from each hit and test which base yields a usable group pointer.
    let mut found = Vec::new();
    for hit in hits {
        for back in 0..4096u32 {
            let Some(base) = hit.checked_sub(back) else {
                break;
            };
            let Ok(bytes) = runtime.read(base + GROUP_IN_CALL, 4) else {
                continue;
            };
            let group = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            // A heap pointer, not text and not a small constant.
            // Reject a group that points at unallocated memory: an all-zero
            // or ASCII-looking word there means the base is wrong, and reading
            // zeros from arbitrary memory is what makes a bogus candidate look
            // like a confirmed null.
            let structured = runtime
                .read(group, 16)
                .map(|b| b.iter().any(|&x| x != 0) && b.iter().any(|&x| !(0x20..0x7f).contains(&x)))
                .unwrap_or(false);
            if group > 0x10000 && group < 0x4000000 && group.is_multiple_of(4) && structured {
                let Ok(bytes) = runtime.read(group + SELF_IN_GROUP, 4) else {
                    continue;
                };
                let selfp = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                // `group[12]` decides the branch in
                // `reset_group_and_self_participant`: zero means it creates the
                // self participant, non-zero means it expects one to exist
                // already and only reads `[592]`. Measuring it says which of
                // "never created" and "created then lost" actually happened.
                let count = runtime
                    .read(group + 12, 4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .unwrap_or(u32::MAX);
                // The offset of the call id inside the object is fixed, so a
                // recurring `back` across candidates is the sign of a real
                // object rather than of a lucky read.
                found.push((base, group, selfp, count, back));
                break;
            }
        }
    }
    found
}

/// Reads the self participant out of a call context, given its base.
///
/// No validated base exists yet, which is the whole difficulty. `0x6d03b4` came
/// out of the scan below and reproduced across two runs, and it is *wrong*:
/// read directly it yields a group pointer of `0xbb4342f4`, three billion, far
/// outside an 18 MiB memory. Two runs agreeing on an address only shows the
/// allocator is deterministic enough to put the call id in the same place — it
/// does not show the address is the object.
///
/// So this takes the base as an argument, and the caller has to have earned it.
/// Ways to earn it, none done yet: derive the call id's offset from the object
/// layout rather than by walking backwards; require several fields to agree
/// with what `getCallInfo` reports at the same moment; or find where
/// `wa_call_start_internal` (10425) gets its first argument.
fn sample(runtime: &mut Runtime, base: u32, when: &str) {
    let Ok(group) = runtime
        .read(base + GROUP_IN_CALL, 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    else {
        println!("  [{when}] group unreadable");
        return;
    };
    // Anything outside linear memory means the base is not a call context.
    if group > 0x4000000 {
        println!("  [{when}] group={group:#x} — not a pointer; base is wrong");
        return;
    }
    let selfp = runtime
        .read(group + SELF_IN_GROUP, 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(u32::MAX);
    println!("  [{when}] group={group:#x} self={selfp:#x}");
}

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let bytes = std::fs::read(&catalog.resolve("JgwtTQVeWPm")?.path)?;

    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    // Same regime as `outgoing_call`: registered as the main runtime thread with
    // `can_block = 0`, so measurements taken here describe the run that example
    // produces rather than a differently-configured one.
    runtime.set_main_thread_registration(true);
    runtime.run_ctors()?;
    runtime.attach_log_ring(4 << 20)?;

    println!(
        "table right after ctors: filled {:?}",
        runtime.table_filled()
    );
    dump_globals(&mut runtime, "before init");

    runtime.call_embind(
        "initVoipStack",
        &[
            Value::Str(SELF.into()),
            Value::Str(SELF_DEVICE.into()),
            Value::Str(SELF_LID.into()),
        ],
    )?;
    runtime.refuel();
    // meowmeow-node-wasm waits for the stack to announce itself before placing
    // a call — `waitForVoipStackReady`, resolved by an `onVoipReady` callback
    // or a 3 s fallback "for builds where onVoipReady never fires". We were
    // calling straight into `startVoipCall`.
    runtime.settle(std::time::Duration::from_secs(3));
    dump_globals(&mut runtime, "after init");
    // Same accessor, before any call: distinguishes a field the call populates
    // from one that is simply always there.
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        println!("after init: context = {ctx:#x}");
        if ctx > 0x10000 {
            sample(&mut runtime, ctx, "after init");
        }
    }

    // Force the guard at offer.cc:430 open. It fails when `arg2 == 0` and the
    // byte at `context + 662166` is zero; `make_and_cache_offer` only ever
    // reads that byte, so whatever sets it lives elsewhere and never runs here.
    // Setting it directly says whether that guard is really what stops the
    // offer, and what the engine does once past it.
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if ctx > 0x10000 {
            let before = runtime.read(ctx + 662_166, 1).ok();
            let _ = runtime.write_bytes_at(ctx + 662_166, &[1]);
            let after = runtime.read(ctx + 662_166, 1).ok();
            println!("forced ctx[662166]: {before:?} -> {after:?}");

            // The guard's *other* term. `wa_call_start_call` calls
            // `wa_call_start_internal` with `(ctx, ctx+248, ctx[3859],
            // ctx[3860], ctx[3856], 0, 0, 0)`, and its parameter 4 is what
            // reaches `make_and_cache_offer` as `arg2` — so `arg2` is the byte
            // at `context + 3856`. Either term being non-zero opens the guard.
            for probe in [3856u32, 3859, 3860] {
                println!("  ctx[{probe}] = {:?}", runtime.read(ctx + probe, 1).ok());
            }
            let _ = runtime.write_bytes_at(ctx + 3856, &[1]);
            println!(
                "  forced ctx[3856] -> {:?}",
                runtime.read(ctx + 3856, 1).ok()
            );
        }
    }

    let mark = runtime.engine_log().len();
    let outcome = runtime.call_embind(
        "startVoipCall",
        &[
            Value::Str(PEER_LID.into()),
            Value::StringList(vec![PEER_LID_DEVICE.to_owned()]),
            Value::Str("0011223344556677".into()),
            // Audio only. A video call was tried, on the theory that `arg2` of
            // `make_and_cache_offer` — local 4 at the call site, "length or
            // count", with `has_video: 0` beside it in the log — is the video
            // flag, which would open the guard at offer.cc:430. Measured over
            // four runs it makes no difference: 70008 still appears in three of
            // them, all with a healthy context.
            Value::Bool(false),
            // meowmeow-node-wasm passes the peer's *PN* here, not the LID
            // again, and `1` for the flag after it.
            Value::Str("11223344556677@s.whatsapp.net".into()),
            Value::Bool(true),
            Value::Bytes(Vec::new()),
        ],
    );
    // Read the context slot at three moments, to localise in time when it
    // stops holding a pointer: while the call was running, or afterwards while
    // the worker threads drain.
    let peek = |r: &Runtime, when: &str| {
        if let Ok(b) = r.read(1_719_816, 4) {
            println!(
                "  slot@{when} = {:#x}",
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            );
        }
    };
    // The whole static neighbourhood, not just the one slot: 1719816 sits 160
    // bytes past the singleton at 1352680 and 32 past the init guard at
    // 1352808, so a write running off the end of either would land on it.
    // Comparing the region before and after shows whether the context pointer
    // is singled out or caught in a wider overwrite.
    let region = |r: &Runtime, when: &str| {
        if let Ok(b) = r.read(1_352_680, 224) {
            let words: Vec<String> = b
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .filter(|(_, w)| w.iter().any(|&x| x != 0))
                .map(|(i, w)| {
                    format!(
                        "+{}={:#x}",
                        i * 4,
                        u32::from_le_bytes([w[0], w[1], w[2], w[3]])
                    )
                })
                .collect();
            println!("  region@{when}: {}", words.join(" "));
        }
    };
    region(&runtime, "before call");
    peek(&runtime, "right after startVoipCall");
    region(&runtime, "after call");
    // `10535` is `get_participant` (call_membership.cc). It skips its whole
    // search loop when `group->[552]` is zero and then returns null — which is
    // exactly what `offer.cc:485` treats as "participant not found". Measure it.
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if ctx > 0x10000
            && let Ok(g) = runtime.read(ctx + GROUP_IN_CALL, 4)
        {
            let group = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
            if group > 0x10000 {
                println!(
                    "  group {group:#x}: [552] = {:?}  (zero means get_participant never searches)",
                    runtime.read(group + 552, 1).ok()
                );
                // `wa_call_group_get_participant(group, i)` is an array accessor:
                // base `group+44`, capacity 127, 4-byte elements, then a load of
                // `[0]`. So participant i is `*(group + 44 + i*4)`.
                //
                // The search compares `p->[4]` and only when `p->[0]` is
                // non-zero — those two words decide whether the comparison even
                // runs, and against what.
                for i in 0..2u32 {
                    let Ok(w) = runtime.read(group + 44 + i * 4, 4) else {
                        continue;
                    };
                    let p = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
                    if p < 0x10000 {
                        println!("  participant[{i}] = {p:#x} (not a pointer)");
                        continue;
                    }
                    let field = |off: u32| {
                        runtime
                            .read(p + off, 4)
                            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                            .unwrap_or(u32::MAX)
                    };
                    let jid = field(4);
                    let text = runtime
                        .read(jid.wrapping_add(8), 40)
                        .map(|b| {
                            b.iter()
                                .take_while(|&&c| c != 0)
                                .map(|&c| {
                                    if (0x20..0x7f).contains(&c) {
                                        c as char
                                    } else {
                                        '.'
                                    }
                                })
                                .collect::<String>()
                        })
                        .unwrap_or_default();
                    println!(
                        "  participant[{i}] = {p:#x}  [0]={:#x}  [4]={jid:#x}  jid+8 -> {text:?}",
                        field(0)
                    );
                    // Raw bytes of the jid struct `10284` compares. It tests
                    // `a+8` against `b+8`, so the shape of that region is what
                    // decides a match.
                    if let Ok(raw) = runtime.read(jid, 48) {
                        let hex: Vec<String> = raw.iter().map(|b| format!("{b:02x}")).collect();
                        println!("      jid raw: {}", hex.join(" "));
                    }
                }
            }
        }
    }
    // The JID the offer path looks up. `make_and_cache_offer` reads `arg1->[0]`
    // and hands it to `wa_call_participant_jid_get_user_jid`; `arg1` is
    // `wa_call_start_internal`'s arg3, and the only pointer-shaped arguments
    // `wa_call_start_call` builds are `ctx+180` and `ctx+248`. Dump both.
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if ctx > 0x10000 {
            for base in [180u32, 248] {
                let Ok(w) = runtime.read(ctx + base, 4) else {
                    continue;
                };
                let ptr = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
                let text = runtime
                    .read(ptr, 48)
                    .map(|bytes| {
                        bytes
                            .iter()
                            .take_while(|&&c| c != 0)
                            .map(|&c| {
                                if (0x20..0x7f).contains(&c) {
                                    c as char
                                } else {
                                    '.'
                                }
                            })
                            .collect::<String>()
                    })
                    .unwrap_or_default();
                println!("  ctx+{base}: [0]={ptr:#x} -> {text:?}");
            }
        }
    }
    // The OTHER 70008 site. `make_and_cache_offer` has two: offer.cc:463 checks
    // the self participant, and offer.cc:430 fails when `arg2 == 0` and the
    // byte at `arg0 + 662166` is zero. The self-participant reading has been
    // assumed all along; this measures the alternative.
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if ctx > 0x10000
            && let Ok(flag) = runtime.read(ctx + 662_166, 1)
        {
            println!("  offer.cc:430 guard — ctx[662166] = {:#x}", flag[0]);
        }
    }
    runtime.refuel();
    peek(&runtime, "after refuel");
    runtime.settle(std::time::Duration::from_secs(5));
    peek(&runtime, "after settle");
    println!("startVoipCall -> {outcome:?}");
    // The whole point: did anything reach the wire?
    println!(
        "SIGNALING SENT: {}  |  call events: {}",
        runtime.all_calls_to("env::sendSignalingXMPP_js_sync").len(),
        runtime.all_calls_to("env::on_call_event_js_sync").len()
    );
    dump_globals(&mut runtime, "after startVoipCall");

    // Static addresses identified from the call path: the singleton guarded by
    // `10598`, its init guard, the lock `10399` wraps, and the two function
    // pointers `8669` reads. If the call context is reachable at all without
    // scanning, it is from one of these.
    // Find the group by who points at the self participant, rather than by
    // guessing a base. The participant struct must hold our own LID, so scan
    // for that, then scan for any word equal to a hit: `group[592]` is defined
    // to be that pointer, so an address A holding it implies `group = A - 592`.
    //
    // If nothing anywhere points at the participant from offset 592, then the
    // cache really is unset everywhere — which is the claim under test.
    // Decisive form: look for *any* base B where `B+659164` holds a pointer G
    // and `G+592` is non-null. That is exactly the shape the offer path
    // demands, so if no such B exists anywhere in memory, the self participant
    // cache is unset everywhere — which is the claim, tested directly instead
    // of through a base guessed from the call id.
    println!("--- any group with a non-null self participant? ---");
    {
        let mut whole = Vec::new();
        let mut at = 0u32;
        while at < (16 << 20) {
            let Ok(block) = runtime.read(at, 1 << 20) else {
                break;
            };
            whole.extend_from_slice(&block);
            at += 1 << 20;
        }
        let word_at = |a: usize| -> Option<u32> {
            whole
                .get(a..a + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let plausible =
            |v: u32| v > 0x10000 && (v as usize) + 596 < whole.len() && v.is_multiple_of(4);

        let mut found = 0usize;
        for base in (0..whole.len().saturating_sub(659_168)).step_by(4) {
            let Some(group) = word_at(base + 659_164) else {
                continue;
            };
            if !plausible(group) {
                continue;
            }
            let Some(selfp) = word_at(group as usize + 592) else {
                continue;
            };
            if selfp != 0 && plausible(selfp) {
                // The shape alone is worthless — memory is full of structs with
                // a non-null word 592 bytes in, and every candidate found that
                // way so far has been one. A real self participant carries our
                // own LID, so require that before reporting anything.
                let window = whole
                    .get(selfp as usize..(selfp as usize + 512).min(whole.len()))
                    .unwrap_or(&[]);
                if !window.windows(14).any(|w| w == b"99887766554433") {
                    continue;
                }
                println!(
                    "  base {:#x} -> group {group:#x} -> self {selfp:#x}  (carries our LID)",
                    base as u32
                );
                found += 1;
            }
        }
        if found == 0 {
            println!("  none — no group anywhere has a non-null self participant");
        }
    }

    // The call context, from the engine's own accessor rather than a guess.
    // `getCallInfo` (embind) is function 1108, which calls 10386, which is
    // seven instructions long and opens with `i32.const 1719816 / i32.load` —
    // so the context pointer is stored *at* 1719816, and this is how the engine
    // itself reaches it.
    println!("--- context via the engine's own accessor ---");
    {
        const CONTEXT_SLOT: u32 = 1_719_816;
        match runtime
            .read(CONTEXT_SLOT, 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        {
            Ok(ctx) => {
                println!("  *[{CONTEXT_SLOT}] = {ctx:#x}");
                if ctx > 0x10000 {
                    sample(&mut runtime, ctx, "context");
                    if let Ok(b) = runtime.read(ctx + GROUP_IN_CALL, 4) {
                        let group = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                        if let Ok(p) = runtime.read(group + SELF_IN_GROUP, 4) {
                            let selfp = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
                            println!(
                                "  => group {group:#x}, group[592] = {selfp:#x}  {}",
                                if selfp == 0 {
                                    "NULL — this is the blocker"
                                } else {
                                    "SET"
                                }
                            );
                        }
                    }
                }
            }
            Err(e) => println!("  unreadable: {e}"),
        }
    }

    // Every JID string the engine holds. The key the lookup searches for must
    // be one of them, and the two the participants carry are known — so a third
    // distinct one is the candidate for what is being searched.
    // `startVoipCall` builds `params` and sets `params->[0] = local8`, where
    // local8 comes from `global.get 10` — measured as 0x18. If that really is
    // the base, the searched JID is reachable from address 0x18.
    // `call->[1288]` is the JID `reset_group_and_self_participant` hands to
    // `wa_call_group_create_participant` when it creates the *self*
    // participant — and the engine reports it failing with "participant
    // 6677@lid already exists", which is the peer. Read it.
    println!("--- call->[1288], the jid used to create SELF ---");
    if let Ok(b) = runtime.read(1_719_816, 4) {
        let ctx = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if ctx > 0x10000
            && let Ok(w) = runtime.read(ctx + 1288, 4)
        {
            let jid = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
            println!("  ctx[1288] = {jid:#x}");
            // Same pj_str_t layout as the participants: {ptr,len} at +8.
            if jid > 0x10000
                && let Ok(raw) = runtime.read(jid, 96)
            {
                let word =
                    |o: usize| u32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
                // Every {ptr,len} pair in the struct, not just the two known
                // ones: if any field holds the 18-char user form, then the
                // conversion exists and `10297` is reading the wrong one.
                for off in (0..88).step_by(8) {
                    let (ptr, len) = (word(off), word(off + 8));
                    if ptr > 0x10000 && (1..=64).contains(&len) {
                        let text = runtime
                            .read(ptr, len)
                            .map(|t| String::from_utf8_lossy(&t).to_string())
                            .unwrap_or_default();
                        println!("    +{off}: len={len} {text:?}");
                    }
                }
            }
        }
    }

    // Where each guest thread's stack starts.
    //
    // Threads are separate instances over one shared memory and the stack
    // pointer is a per-instance global, so equal values here mean they are all
    // writing over the same region — which would explain a pointer into a
    // caller's frame coming back cleared.
    let stacks: Vec<String> = runtime
        .logs()
        .into_iter()
        .filter(|line| line.contains("stack pointer"))
        .collect();
    println!("--- thread stacks ({} reported) ---", stacks.len());
    for line in &stacks {
        println!("  {}", line.trim());
    }

    println!(
        "main thread table: size {:?} filled {:?}",
        runtime.table_size(),
        runtime.table_filled()
    );

    println!("--- low memory around 0x18 ---");
    if let Ok(b) = runtime.read(0, 64) {
        for (i, w) in b.as_chunks::<4>().0.iter().enumerate() {
            let v = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
            if v != 0 {
                println!("  [{}] = {v:#x}", i * 4);
            }
        }
    }

    // Follow whatever a patched module parked in the low scratch words.
    //
    // The words themselves are not reliably free — an unpatched run already has
    // values in some of them — so this prints what it followed rather than
    // asserting the pointer is ours. A plausible heap address that dereferences
    // to structure is worth seeing; anything else shows up as obvious garbage.
    for slot in [24u32, 28, 36] {
        let Ok(word) = runtime.read(slot, 4) else {
            continue;
        };
        let ptr = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        if !(0x10_000..0x400_0000).contains(&ptr) {
            continue;
        }
        println!("--- following *({slot}) = {ptr:#x} ---");
        if let Ok(block) = runtime.read(ptr, 64) {
            for (i, w) in block.as_chunks::<4>().0.iter().enumerate() {
                let v = u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
                // What an entry *is* decides the diagnosis: a struct pointer and
                // a `char*` are both plausible-looking words, and the engine
                // dereferences one of them as the other if the host hands it the
                // wrong kind. Showing the text settles it without a second run.
                let text = runtime
                    .read(v, 24)
                    .ok()
                    .filter(|b| b.first().is_some_and(|c| (0x20u8..0x7f).contains(c)))
                    .map(|b| {
                        b.iter()
                            .copied()
                            .take_while(|c| (0x20u8..0x7f).contains(c))
                            .map(char::from)
                            .collect::<String>()
                    })
                    .filter(|t| t.len() >= 4);
                match text {
                    Some(t) => println!("  +{:<3} = {v:#x}  -> {t:?}", i * 4),
                    None => println!("  +{:<3} = {v:#x}", i * 4),
                }
            }
        }
    }

    println!("--- distinct JID strings in memory ---");
    {
        let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        let mut at = 0u32;
        while at < (16 << 20) {
            let Ok(block) = runtime.read(at, 1 << 20) else {
                break;
            };
            for suffix in [
                b"@lid".as_slice(),
                b"@s.whatsapp.net".as_slice(),
                b"@c.us".as_slice(),
            ] {
                for (off, w) in block.windows(suffix.len()).enumerate() {
                    if w != suffix {
                        continue;
                    }
                    // Walk back over the local part.
                    let mut start = off;
                    while start > 0 {
                        let c = block[start - 1];
                        if c.is_ascii_digit() || c == b':' || c == b'.' || c == b'-' {
                            start -= 1;
                        } else {
                            break;
                        }
                    }
                    if start == off {
                        continue;
                    }
                    let text =
                        String::from_utf8_lossy(&block[start..off + suffix.len()]).to_string();
                    *seen.entry(text).or_default() += 1;
                }
            }
            at += (1 << 20) - 32;
        }
        for (jid, count) in seen.iter().take(20) {
            println!("  {jid}  x{count}");
        }
    }

    println!("--- static candidates ---");
    for addr in [1_352_680u32, 1_352_808, 1_354_760, 1_263_140, 1_263_144] {
        let word = runtime
            .read(addr, 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        match word {
            Ok(value) => {
                let plausible = value > 0x10000 && value < 0x4000000;
                println!(
                    "  [{addr}] = {value:#x}{}",
                    if plausible {
                        "  (in-memory pointer)"
                    } else {
                        ""
                    }
                );
                // If it is a pointer, does it look like a call context?
                if plausible && let Ok(b) = runtime.read(value + GROUP_IN_CALL, 4) {
                    let group = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    if group > 0x10000 && group < 0x4000000 {
                        println!("      +659164 -> {group:#x}  <-- candidate context!");
                    }
                }
            }
            Err(_) => println!("  [{addr}] unreadable"),
        }
        // A statically-initialised singleton lives *at* its address rather than
        // being pointed to by it, so the address itself is a candidate base.
        if let Ok(b) = runtime.read(addr + GROUP_IN_CALL, 4) {
            let group = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if group > 0x10000 && group < 0x4000000 {
                let selfp = runtime
                    .read(group + SELF_IN_GROUP, 4)
                    .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
                    .unwrap_or(u32::MAX);
                println!(
                    "      as base: +659164 -> group {group:#x}, group[592] = {selfp:#x}  <-- CANDIDATE"
                );
            }
        }
    }

    println!("--- scanning for the call context ---");
    for (base, group, selfp, count, back) in find_context(&mut runtime, "0011223344556677") {
        println!(
            "  candidate call={base:#x} id@+{back} group={group:#x} group[12]={count:#x} group[592]={selfp:#x} {}",
            match (count, selfp) {
                (0, 0) => "<-- would CREATE self, yet null",
                (_, 0) => "<-- expected an existing self, found null",
                _ => "<-- self set",
            }
        );
    }
    // Same reads, but through the guard: a base whose group is outside linear
    // memory is reported as wrong rather than silently producing numbers.
    for (base, ..) in find_context(&mut runtime, "0011223344556677") {
        sample(&mut runtime, base, &format!("re-read {base:#x}"));
    }

    // Say how much is being hidden before hiding it.
    //
    // The filter below is a display choice, and reading its output as the whole
    // log makes this run look like it takes a far shorter path than it does —
    // which reads as "that code never ran" when the code ran and simply said
    // nothing matching. Print the count first so the two cannot be confused.
    let lines = runtime.engine_log_from(mark);
    let shown: Vec<_> = lines
        .iter()
        .filter(|line| line.contains("make_and_cache_offer") || line.contains("Calling"))
        .collect();
    println!(
        "--- engine log: {} lines, showing the {} about the call ---",
        lines.len(),
        shown.len()
    );
    for line in shown {
        println!("  | {}", line.trim());
    }

    Ok(())
}
