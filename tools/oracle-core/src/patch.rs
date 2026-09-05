//! Rewriting a captured module so a run can be traced.
//!
//! Reading a module says what it *could* do. This says what it *did*: it splices
//! calls to an observable import into chosen places, and the host's own call
//! trace then reports which of them were reached, in order. It is what closes
//! the gap the disassembly cannot — `oracle abi` shows ten call sites for a
//! function and nothing about which one ran.
//!
//! ## Why a body patch is not enough, and this is
//!
//! `agent_docs/voip_oracle_status.md` measured the null-key chain by patching the *bodies* of
//! `10297` and `10535`, and says why the result is weak: those functions have
//! ten and three call sites, so a body patch reports whichever call happened to
//! run rather than the one on the path being traced. Marking the *call site*
//! answers the question the body patch cannot.
//!
//! ## How it avoids renumbering anything
//!
//! A marker is a call to an import the module already declares, named by the
//! caller or taken from [`RECORDING_ONLY_SINKS`]. Adding a new import would
//! shift every function index by one and require rewriting every `call` in the
//! module; reusing one costs nothing. Ids start high (see [`Plan::id_base`]) so
//! a marker is never mistaken for a real argument.
//!
//! ## Why the sink is not simply the first import of the right shape
//!
//! It was, and that is not behaviour-neutral. `(i32, i32) -> ()` is the shape of
//! `env::get_random_bytes_js`, which takes `(len, buf)` and *writes*: a marker
//! calling it turns the marker id into a length and fills guest memory from
//! address zero. It is also the shape of `env::_embind_register_void`, which
//! mutates the type registry. Either way the instrumented module still
//! validates, still runs, and reports a trace of a program that is not the one
//! being traced — the exact failure this module exists to make impossible. So
//! the choice is a short list of imports this host is known to answer without
//! touching the guest, and anything else has to be named deliberately.
//!
//! Bodies are spliced as **raw bytes** rather than decoded and re-encoded. That
//! is sound for a specific reason: a wasm body contains no absolute byte
//! offsets. Branches carry relative label depths, `br_table` carries label
//! indices, and neither moves when straight-line code is inserted ahead of
//! them. The inserted sequence is stack-neutral — it pushes two values and the
//! call consumes both — so it is well-typed anywhere the stack is, including
//! between a callee's arguments and its `call`.
//!
//! What does move is the two length prefixes, so the code section is re-emitted
//! whole: every body keeps its bytes and gets a fresh size, which absorbs a
//! prefix that grew from one LEB128 byte to two.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow, ensure};
use wasmparser::{Operator, Parser, Payload, TypeRef};

/// Where a marker was placed, and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// The value the marker passes as its first argument.
    pub id: i32,
    /// What kind of site this is: `entry`, `before-call`, `return`.
    pub kind: String,
    /// The function the marker sits in.
    pub func: u32,
    /// Human-readable detail — for a call site, the callee.
    pub detail: String,
}

/// Everything needed to read a captured run back into source locations.
#[derive(Debug, Clone)]
pub struct MarkerMap {
    /// The import index every marker calls.
    pub via_import: u32,
    /// The import's `module::name`, which is what the host trace records.
    pub via_symbol: String,
    /// Every marker placed, in the order the ids were handed out.
    pub markers: Vec<Marker>,
}

impl MarkerMap {
    /// The marker with this id, if it is one of ours.
    #[must_use]
    pub fn get(&self, id: i32) -> Option<&Marker> {
        self.markers.iter().find(|marker| marker.id == id)
    }

    /// Turns the host's recorded calls into the markers that fired, in order.
    ///
    /// Calls to the sink import that carry an id outside this map are the
    /// module's own use of it and are skipped — the sink is a real import with
    /// a real caller, which is the price of not renumbering the module.
    #[must_use]
    pub fn fired<'a>(
        &'a self,
        calls: impl IntoIterator<Item = (&'a str, &'a [i64])>,
    ) -> Vec<(&'a Marker, i64)> {
        calls
            .into_iter()
            .filter(|(symbol, _)| *symbol == self.via_symbol)
            .filter_map(|(_, args)| {
                let id = i32::try_from(*args.first()?).ok()?;
                let value = args.get(1).copied().unwrap_or(0);
                Some((self.get(id)?, value))
            })
            .collect()
    }

    /// Markers that never fired. The absence is the finding as often as the
    /// presence is: a call site that was never reached says the path taken was
    /// not the one being traced.
    #[must_use]
    pub fn never_fired<'a>(&'a self, fired: &[(&Marker, i64)]) -> Vec<&'a Marker> {
        self.markers
            .iter()
            .filter(|marker| !fired.iter().any(|(hit, _)| hit.id == marker.id))
            .collect()
    }
}

/// What to instrument.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Mark the first instruction of each of these functions.
    pub entry: Vec<u32>,
    /// Mark the entry of `.0`, reporting local `.1` as the marker's value.
    ///
    /// For a parameter this is the argument it was called with, which is the
    /// difference between "`free` trapped" and "`free` trapped on `0x2a6820`,
    /// which is in the stack". A local rather than a memory read because a
    /// parameter is always in scope at entry and needs no address to be right.
    pub value_entry: Vec<(u32, u32)>,
    /// Report an i32 local immediately before an instruction (zero-based
    /// operator ordinal, excluding local declarations) in the original body.
    pub value_at: Vec<(u32, usize, u32, bool)>,
    /// Mark every call site in `.0` that targets `.1`. A `None` callee marks
    /// *every* direct call the function makes, which is the shape to reach for
    /// when the question is "how far did it get".
    pub before_calls: Vec<(u32, Option<u32>)>,
    /// Mark every `return` in each of these functions.
    pub at_returns: Vec<u32>,
    /// The first marker id. Well clear of anything the module passes for real:
    /// `cargo xt oracle tag-offer-error-sites` used the same base for the same reason.
    pub id_base: i32,
    /// The import to call, as `module::name` or as a bare name.
    ///
    /// `None` picks the first candidate on [`RECORDING_ONLY_SINKS`], and
    /// refuses when there is none. Naming one is the caller saying it has
    /// checked what that import does to the guest — see the note at the top of
    /// this module for what happens when it does something.
    pub sink: Option<String>,
}

impl Plan {
    /// A plan that marks every direct call site in one function.
    #[must_use]
    pub fn every_call_in(func: u32) -> Self {
        Self {
            before_calls: vec![(func, None)],
            id_base: DEFAULT_ID_BASE,
            ..Self::default()
        }
    }
}

/// Where marker ids start by default.
pub const DEFAULT_ID_BASE: i32 = 200_000;

/// Imports `oracle-core` answers without changing anything the guest can see,
/// so a spliced call to one cannot alter what the module computes.
///
/// Each entry is here because of what the *host* does with it, not because of
/// its name — which is why this list is short and why adding to it means
/// reading the handler first:
///
/// - `env::on_call_event_js_sync` — WhatsApp's own callback. Nothing in this
///   crate defines it, so `host::define_stubs` answers it: record the
///   arguments, return nothing.
/// - `env::loggingCallback_js_sync` — defined, but it only records the call and
///   reads the guest (`read_sized`, then `log`). A marker adds a junk log line
///   and nothing else. Note what a `value_entry` marker costs here: the value
///   is a guest local, this import reads it as a length, and an out-of-range
///   one comes back empty rather than failing — so the noise is bounded but
///   the log is the thing that pays for it.
/// - `env::mark` — the conventional name for an import a module carries for
///   exactly this purpose; the fixtures in `tests/patch.rs` use it.
///
/// Everything else has to be named through [`Plan::sink`].
pub const RECORDING_ONLY_SINKS: &[&str] = &[
    "env::on_call_event_js_sync",
    "env::loggingCallback_js_sync",
    "env::mark",
];

/// One instruction-level replacement, for forcing a branch or neutralising a
/// call while tracing.
#[derive(Debug, Clone)]
pub struct Replace {
    /// The function to edit.
    pub func: u32,
    /// The operator's index in the body, counting from zero — the same order
    /// `oracle abi --index` prints.
    pub at: usize,
    /// How many operators to replace.
    pub count: usize,
    /// What to put there. Must leave the stack as it found it.
    pub with: Vec<Edit>,
}

/// The instructions a [`Replace`] may write. Deliberately few: this exists to
/// force a gate open or drop a value, not to author code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// `i32.const n`
    I32(i32),
    /// `i64.const n`
    I64(i64),
    /// `drop`
    Drop,
    /// `nop`
    Nop,
}

impl Edit {
    fn encode(self, sink: &mut Vec<u8>) {
        use wasm_encoder::{Encode, Instruction};
        match self {
            Self::I32(value) => Instruction::I32Const(value).encode(sink),
            Self::I64(value) => Instruction::I64Const(value).encode(sink),
            Self::Drop => Instruction::Drop.encode(sink),
            Self::Nop => Instruction::Nop.encode(sink),
        }
    }
}

/// Parses `FUNC:AT:COUNT:SPEC`, where SPEC is `;`-separated.
///
/// # Errors
///
/// Returns an error when a field is missing or an instruction is not one of the
/// four [`Edit`] understands.
pub fn parse_replace(text: &str) -> Result<Replace> {
    let mut fields = text.splitn(4, ':');
    let mut next = |what: &str| -> Result<&str> {
        fields
            .next()
            .filter(|field| !field.is_empty())
            .with_context(|| {
                format!("replacement is missing its {what}: expected FUNC:AT:COUNT:SPEC")
            })
    };

    let func = next("function")?.parse().context("function index")?;
    let at = next("position")?.parse().context("position")?;
    let count = next("count")?.parse().context("count")?;
    let spec = next("instructions")?;

    let mut with = Vec::new();
    for token in spec.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        with.push(match token {
            "drop" => Edit::Drop,
            "nop" => Edit::Nop,
            _ => {
                if let Some(value) = token.strip_prefix("i32.const ") {
                    Edit::I32(value.trim().parse().context("i32.const value")?)
                } else if let Some(value) = token.strip_prefix("i64.const ") {
                    Edit::I64(value.trim().parse().context("i64.const value")?)
                } else {
                    return Err(anyhow!(
                        "unsupported instruction `{token}`; use drop, nop, `i32.const N` or `i64.const N`"
                    ));
                }
            }
        });
    }

    Ok(Replace {
        func,
        at,
        count,
        with,
    })
}

/// What the parse of a module turned up, so the rewrite can work in bytes.
struct Layout {
    /// Number of imported functions, which is the index the first defined one
    /// takes.
    imported_funcs: u32,
    /// Every import with the sink signature, as (function index,
    /// `module::name`), in declaration order. All of them, not the first —
    /// choosing is [`choose_sink`]'s job, and a refusal has to be able to name
    /// the ones it rejected.
    sinks: Vec<(u32, String)>,
    /// Byte range of the whole code section, including its id and size prefix.
    code_section: std::ops::Range<usize>,
    /// Byte range of each defined function's body contents, in index order.
    bodies: Vec<std::ops::Range<usize>>,
    /// Where the instructions start in each body, past its locals.
    code_starts: Vec<usize>,
}

/// The signature a marker sink must have: two `i32` in, nothing out.
fn is_sink(ty: &wasmparser::FuncType) -> bool {
    ty.params().len() == 2
        && ty.results().is_empty()
        && ty
            .params()
            .iter()
            .all(|param| *param == wasmparser::ValType::I32)
}

fn read_layout(bytes: &[u8]) -> Result<Layout> {
    let mut types: Vec<wasmparser::FuncType> = Vec::new();
    let mut imported_funcs = 0u32;
    let mut sinks = Vec::new();
    let mut code_section = 0..0;
    let mut bodies = Vec::new();
    let mut code_starts = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.context("parsing module")? {
            Payload::TypeSection(section) => {
                for group in section.into_iter() {
                    for sub in group.context("type group")?.into_types() {
                        if let wasmparser::CompositeInnerType::Func(func) = sub.composite_type.inner
                        {
                            types.push(func);
                        }
                    }
                }
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.context("reading import")?;
                    let (TypeRef::Func(index) | TypeRef::FuncExact(index)) = import.ty else {
                        continue;
                    };
                    // Every import with the sink signature. Which one is used
                    // very much does matter — see the note at the top of this
                    // module — so the choice is made later, from names.
                    if types.get(index as usize).is_some_and(is_sink) {
                        sinks.push((
                            imported_funcs,
                            format!("{}::{}", import.module, import.name),
                        ));
                    }
                    imported_funcs += 1;
                }
            }
            Payload::CodeSectionStart { range, .. } => {
                // `range` covers the entries; back up over the id and size so
                // the whole section can be replaced as a unit.
                code_section = range;
            }
            Payload::CodeSectionEntry(body) => {
                let range = body.range();
                let start = body
                    .get_operators_reader()
                    .context("reading a function body")?
                    .original_position();
                bodies.push(range);
                code_starts.push(start);
            }
            _ => {}
        }
    }

    if bodies.is_empty() {
        return Err(anyhow!("module has no code section to instrument"));
    }

    Ok(Layout {
        imported_funcs,
        sinks,
        code_section,
        bodies,
        code_starts,
    })
}

/// Picks the import a marker will call, or explains why it will not pick one.
///
/// `wanted` is what [`Plan::sink`] asked for: a full `module::name`, or a bare
/// name when the caller does not want to spell the import module out. Matching
/// on the bare name is deliberately allowed and deliberately strict about the
/// result — two imports of the same name in different modules is ambiguous, and
/// an ambiguous sink is refused rather than resolved, for the same reason a
/// fingerprint two names share is dropped.
fn choose_sink(candidates: &[(u32, String)], wanted: Option<&str>) -> Result<(u32, String)> {
    if candidates.is_empty() {
        return Err(anyhow!(
            "module declares no (i32, i32) -> () import to use as a marker sink; \
             instrumenting would mean adding one, which renumbers every function"
        ));
    }

    let names = || {
        candidates
            .iter()
            .map(|(_, symbol)| symbol.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    if let Some(wanted) = wanted {
        let matched: Vec<&(u32, String)> = candidates
            .iter()
            .filter(|(_, symbol)| {
                symbol == wanted || symbol.rsplit_once("::").is_some_and(|(_, n)| n == wanted)
            })
            .collect();
        return match matched.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(anyhow!(
                "`{wanted}` is not an (i32, i32) -> () import of this module; it declares {}",
                names()
            )),
            many => Err(anyhow!(
                "`{wanted}` is ambiguous: it names {} of this module's imports",
                many.len()
            )),
        };
    }

    candidates
        .iter()
        .find(|(_, symbol)| RECORDING_ONLY_SINKS.contains(&symbol.as_str()))
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "none of this module's (i32, i32) -> () imports is known to be recording-only, \
                 and calling one that is not can change what the module computes: it declares \
                 {}. Name one through `Plan::sink` (`--sink`) once you have read what the host \
                 does with it.",
                names()
            )
        })
}

/// A byte to insert at an absolute file offset.
struct Splice {
    at: usize,
    bytes: Vec<u8>,
}

/// The three instructions a marker is.
fn marker_bytes(id: i32, value: i32, sink: u32) -> Vec<u8> {
    use wasm_encoder::{Encode, Instruction};
    let mut bytes = Vec::new();
    Instruction::I32Const(id).encode(&mut bytes);
    Instruction::I32Const(value).encode(&mut bytes);
    Instruction::Call(sink).encode(&mut bytes);
    bytes
}

/// The same, reporting a local instead of a constant.
fn marker_bytes_local(id: i32, local: u32, sink: u32) -> Vec<u8> {
    use wasm_encoder::{Encode, Instruction};
    let mut bytes = Vec::new();
    Instruction::I32Const(id).encode(&mut bytes);
    Instruction::LocalGet(local).encode(&mut bytes);
    Instruction::Call(sink).encode(&mut bytes);
    bytes
}

/// Instruments a module, returning the new bytes and the map to read a run with.
///
/// # Errors
///
/// Returns an error when the module declares no import that can serve as a
/// marker sink — including when it declares one whose host implementation is
/// not known to be recording-only, see `choose_sink` — when it has no code
/// section, or when a named function does not exist.
pub fn instrument(bytes: &[u8], plan: &Plan) -> Result<(Vec<u8>, MarkerMap)> {
    let layout = read_layout(bytes)?;
    let (sink, sink_symbol) = choose_sink(&layout.sinks, plan.sink.as_deref())?;

    let mut markers = Vec::new();
    let mut splices: Vec<Splice> = Vec::new();
    let mut next_id = if plan.id_base == 0 {
        DEFAULT_ID_BASE
    } else {
        plan.id_base
    };

    let body_of = |func: u32| -> Result<usize> {
        let ordinal = func
            .checked_sub(layout.imported_funcs)
            .and_then(|ordinal| usize::try_from(ordinal).ok())
            .filter(|ordinal| *ordinal < layout.bodies.len())
            .ok_or_else(|| {
                anyhow!(
                    "function {func} is not a defined function in this module \
                     ({} imported, {} defined)",
                    layout.imported_funcs,
                    layout.bodies.len()
                )
            })?;
        Ok(ordinal)
    };

    for &func in &plan.entry {
        let ordinal = body_of(func)?;
        splices.push(Splice {
            at: layout.code_starts[ordinal],
            bytes: marker_bytes(next_id, 0, sink),
        });
        markers.push(Marker {
            id: next_id,
            kind: "entry".to_owned(),
            func,
            detail: format!("entry of func {func}"),
        });
        next_id += 1;
    }

    for &(func, local) in &plan.value_entry {
        let ordinal = body_of(func)?;
        splices.push(Splice {
            at: layout.code_starts[ordinal],
            bytes: marker_bytes_local(next_id, local, sink),
        });
        markers.push(Marker {
            id: next_id,
            kind: "value".to_owned(),
            func,
            detail: format!("local {local} at entry of func {func}"),
        });
        next_id += 1;
    }

    for &(func, instruction, local, float) in &plan.value_at {
        let ordinal = body_of(func)?;
        let start = layout.code_starts[ordinal];
        let end = layout.bodies[ordinal].end;
        let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
            &bytes[start..end],
            start,
        ));
        let (_, offset) = reader
            .into_iter_with_offsets()
            .nth(instruction)
            .with_context(|| format!("func {func} has no instruction {instruction}"))??;
        let mut marker = Vec::new();
        use wasm_encoder::{Encode, Instruction};
        Instruction::I32Const(next_id).encode(&mut marker);
        Instruction::LocalGet(local).encode(&mut marker);
        if float {
            Instruction::I32ReinterpretF32.encode(&mut marker);
        }
        Instruction::Call(sink).encode(&mut marker);
        splices.push(Splice {
            at: offset,
            bytes: marker,
        });
        markers.push(Marker {
            id: next_id,
            kind: "value-at".into(),
            func,
            detail: format!("local {local} before instruction {instruction}"),
        });
        next_id += 1;
    }

    for &(func, only) in &plan.before_calls {
        let ordinal = body_of(func)?;
        let body = &bytes[layout.bodies[ordinal].clone()];
        let base = layout.bodies[ordinal].start;
        let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
            &body[layout.code_starts[ordinal] - base..],
            layout.code_starts[ordinal],
        ));

        for item in reader.into_iter_with_offsets() {
            let (op, offset) = item.context("reading operators")?;
            let Operator::Call { function_index } = op else {
                continue;
            };
            if only.is_some_and(|wanted| wanted != function_index) {
                continue;
            }
            splices.push(Splice {
                at: offset,
                bytes: marker_bytes(next_id, 0, sink),
            });
            markers.push(Marker {
                id: next_id,
                kind: "before-call".to_owned(),
                func,
                detail: format!("call {function_index} in func {func}"),
            });
            next_id += 1;
        }
    }

    for &func in &plan.at_returns {
        let ordinal = body_of(func)?;
        let body = &bytes[layout.bodies[ordinal].clone()];
        let base = layout.bodies[ordinal].start;
        let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
            &body[layout.code_starts[ordinal] - base..],
            layout.code_starts[ordinal],
        ));

        for item in reader.into_iter_with_offsets() {
            let (op, offset) = item.context("reading operators")?;
            if !matches!(op, Operator::Return) {
                continue;
            }
            splices.push(Splice {
                at: offset,
                bytes: marker_bytes(next_id, 0, sink),
            });
            markers.push(Marker {
                id: next_id,
                kind: "return".to_owned(),
                func,
                detail: format!("return in func {func}"),
            });
            next_id += 1;
        }
    }

    let rewritten = rewrite(bytes, &layout, splices, &[])?;
    Ok((
        rewritten,
        MarkerMap {
            via_import: sink,
            via_symbol: sink_symbol,
            markers,
        },
    ))
}

/// Applies instruction replacements and returns the new module.
///
/// # Errors
///
/// Returns an error when a function does not exist, when a position is past the
/// end of its body, or when the module has no code section.
pub fn replace(bytes: &[u8], edits: &[Replace]) -> Result<Vec<u8>> {
    let layout = read_layout(bytes)?;
    let mut cuts = Vec::new();

    for edit in edits {
        let ordinal = edit
            .func
            .checked_sub(layout.imported_funcs)
            .and_then(|ordinal| usize::try_from(ordinal).ok())
            .filter(|ordinal| *ordinal < layout.bodies.len())
            .ok_or_else(|| anyhow!("function {} is not defined here", edit.func))?;

        let body = &bytes[layout.bodies[ordinal].clone()];
        let base = layout.bodies[ordinal].start;
        let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
            &body[layout.code_starts[ordinal] - base..],
            layout.code_starts[ordinal],
        ));

        // The offsets of the operators being replaced, and of the one after.
        let mut offsets = Vec::new();
        for item in reader.into_iter_with_offsets() {
            let (_, offset) = item.context("reading operators")?;
            offsets.push(offset);
        }
        let start = *offsets.get(edit.at).ok_or_else(|| {
            anyhow!(
                "func {} has {} operators; there is nothing at {}",
                edit.func,
                offsets.len(),
                edit.at
            )
        })?;
        let end = offsets
            .get(edit.at + edit.count)
            .copied()
            .unwrap_or(layout.bodies[ordinal].end);

        let mut replacement = Vec::new();
        for instruction in &edit.with {
            instruction.encode(&mut replacement);
        }
        cuts.push((start..end, replacement));
    }

    cuts.sort_by_key(|(range, _)| range.start);
    for adjacent in cuts.windows(2) {
        ensure!(
            adjacent[0].0.end <= adjacent[1].0.start,
            "replacement ranges {}..{} and {}..{} overlap",
            adjacent[0].0.start,
            adjacent[0].0.end,
            adjacent[1].0.start,
            adjacent[1].0.end
        );
    }

    rewrite(bytes, &layout, Vec::new(), &cuts)
}

/// Rebuilds the module with the code section re-emitted.
///
/// Splices are insertions at a point; cuts replace a range. Both are expressed
/// in absolute file offsets, and both land inside function bodies.
fn rewrite(
    bytes: &[u8],
    layout: &Layout,
    mut splices: Vec<Splice>,
    cuts: &[(std::ops::Range<usize>, Vec<u8>)],
) -> Result<Vec<u8>> {
    // One ordered list of edits, so the walk below never has to decide between
    // two kinds. An insertion is a zero-width cut, which is what makes them the
    // same shape: several insertions at one offset simply emit in turn.
    let mut edits: Vec<(std::ops::Range<usize>, Vec<u8>)> = Vec::new();
    splices.sort_by_key(|splice| splice.at);
    for splice in splices {
        edits.push((splice.at..splice.at, splice.bytes));
    }
    edits.extend(cuts.iter().cloned());
    edits.sort_by_key(|(range, _)| range.start);

    let mut section = wasm_encoder::CodeSection::new();
    let mut pending = edits.into_iter().peekable();

    for range in &layout.bodies {
        let mut body = Vec::new();
        let mut at = range.start;

        while let Some((edit, _)) = pending.peek() {
            if edit.start >= range.end {
                break;
            }
            let (edit, replacement) = pending.next().expect("just peeked");
            // `replace` rejects overlapping cuts. Instrumentation can place
            // multiple zero-width markers at one point, so keep this guard for
            // the shared rewrite path without discarding any replacement.
            if edit.start < at {
                continue;
            }
            body.extend_from_slice(&bytes[at..edit.start]);
            body.extend_from_slice(&replacement);
            at = edit.end.max(edit.start);
        }

        body.extend_from_slice(&bytes[at..range.end]);
        section.raw(&body);
    }

    let mut out = Vec::with_capacity(bytes.len() + 1024);
    out.extend_from_slice(&bytes[..layout.code_section.start]);
    // `CodeSectionStart::range` covers the entries only, so the id and size
    // prefix sit just before it. Emitting the section through wasm-encoder
    // writes both afresh, which is what absorbs a length prefix that grew.
    let prefix = section_prefix_start(bytes, layout.code_section.start);
    out.truncate(prefix);
    // The id byte is ours to write: `Encode` for a section emits its length and
    // payload, and `Module::section` is what normally prepends the id. Leaving
    // it out puts the new length where the id belongs, and the module then
    // fails to parse with the length reported as a section id.
    out.push(wasm_encoder::SectionId::Code as u8);
    wasm_encoder::Encode::encode(&section, &mut out);
    out.extend_from_slice(&bytes[layout.code_section.end..]);

    Ok(out)
}

/// Walks back from the first code entry to the section's id byte.
///
/// The id is followed by a LEB128 size; both must go, because the section is
/// re-emitted with a size of its own.
fn section_prefix_start(bytes: &[u8], entries_start: usize) -> usize {
    // At most five bytes of LEB128 plus the id.
    let mut at = entries_start.saturating_sub(6);
    while at < entries_start {
        if bytes.get(at) == Some(&0x0A) {
            // Confirm the LEB128 that follows lands exactly on `entries_start`.
            let mut cursor = at + 1;
            while cursor < entries_start && bytes[cursor] & 0x80 != 0 {
                cursor += 1;
            }
            if cursor + 1 == entries_start {
                return at;
            }
        }
        at += 1;
    }
    entries_start
}

/// Counts direct call sites per callee in a function, which is what says
/// whether a body patch could have answered the question at all.
///
/// # Errors
///
/// Returns an error when the module cannot be parsed or the function is not
/// defined here.
pub fn call_sites(bytes: &[u8], func: u32) -> Result<BTreeMap<u32, usize>> {
    let layout = read_layout(bytes)?;
    let ordinal = func
        .checked_sub(layout.imported_funcs)
        .and_then(|ordinal| usize::try_from(ordinal).ok())
        .filter(|ordinal| *ordinal < layout.bodies.len())
        .ok_or_else(|| anyhow!("function {func} is not defined here"))?;

    let range = layout.bodies[ordinal].clone();
    let body = &bytes[range.clone()];
    let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
        &body[layout.code_starts[ordinal] - range.start..],
        layout.code_starts[ordinal],
    ));

    let mut counts = BTreeMap::new();
    for item in reader.into_iter_with_offsets() {
        let (op, _) = item.context("reading operators")?;
        if let Operator::Call { function_index } = op {
            *counts.entry(function_index).or_default() += 1;
        }
    }
    Ok(counts)
}
