//! Inferring what a function's parameters mean, from its own code.
//!
//! A stripped module tells you a signature — `(i32, i32, i32) -> i32` — and
//! nothing about what those integers are. Guessing is how you end up passing a
//! length where a pointer goes and reading a plausible wrong answer, which for
//! an oracle is worse than reading nothing.
//!
//! The code knows, though. A parameter dereferenced as an address is a pointer,
//! and the width of the access says what it points at. One that only ever bounds
//! a loop is a length. One that is passed straight through to another call is a
//! handle. This walks the bytecode and collects that evidence.
//!
//! It needs no name section, no glue and no debug info, so it works on any
//! module — including the minified ones where every export is a single letter.
//!
//! # What it is and is not
//!
//! This is evidence, not proof. It reports what the code does with each
//! parameter; one used in no distinguishing way comes back as `Unknown` rather
//! than as a guess, and `evidence()` prints the raw counts alongside the role so
//! a reader can judge the classification instead of taking it. Two limits are
//! worth knowing before trusting a label:
//!
//! - **The scan is linear**, so it tracks one path through the function. A
//!   parameter only touched inside a branch may come back thinner than it is,
//!   and a slot the optimiser recycled is retired at the first store that does
//!   not write the parameter back — evidence is given up rather than invented.
//!   The analysis classifies arguments; it does not decompile.
//! - **A role is a shape, not a type.** `Handle` covers both an opaque pointer
//!   passed through and an integer written into a config struct, because the
//!   code does the same thing with each. Which one it is comes from what the
//!   callee does with it — which is what `--index` and `--slot` are for.

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use wasmparser::{Operator, Parser, Payload};

/// What a parameter appears to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// Dereferenced as an address. `width` is the widest access seen, which is
    /// usually the size of what it points at.
    Pointer {
        /// Width of the widest access seen, in bytes.
        width: u32,
        /// Whether anything was written through it.
        writes: bool,
    },
    /// Compared or used to bound a loop, and never dereferenced.
    Length,
    /// Passed to another function or written into a structure unchanged, and not
    /// otherwise examined — the shape of an opaque handle or a config value.
    Handle,
    /// Used in arithmetic, but never as an address.
    Scalar,
    /// Nothing distinguishing. Reported rather than guessed at.
    Unknown,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointer { width, writes } => {
                let access = if *writes { "read/write" } else { "read" };
                write!(f, "pointer ({access}, {width}-byte access)")
            }
            Self::Length => f.write_str("length or count"),
            Self::Handle => f.write_str("handle (passed through)"),
            Self::Scalar => f.write_str("scalar"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

/// Evidence gathered about one parameter.
#[derive(Debug, Clone, Default)]
pub struct ParamEvidence {
    /// The parameter's position in the signature.
    pub index: u32,
    /// Widest load through this parameter, in bytes.
    pub load_width: Option<u32>,
    /// Widest store through this parameter, in bytes.
    pub store_width: Option<u32>,
    /// Times it was compared against something.
    pub comparisons: usize,
    /// Times it was passed to another function.
    pub forwarded: usize,
    /// Times it took part in address arithmetic.
    pub arithmetic: usize,
    /// Times it was written into memory as a value rather than used as an
    /// address. A width or a height reaching a config struct looks like this,
    /// and without it such a parameter reads as unused.
    pub stored_into: usize,
}

impl ParamEvidence {
    /// Classifies the parameter from what was seen.
    ///
    /// Dereferencing wins over everything: a pointer that is also compared
    /// against null is still a pointer. Comparison without dereferencing is the
    /// signature of a bound.
    pub fn role(&self) -> Role {
        let width = self.load_width.max(self.store_width);
        if let Some(width) = width {
            return Role::Pointer {
                width,
                writes: self.store_width.is_some(),
            };
        }
        if self.comparisons > 0 {
            return Role::Length;
        }
        if self.forwarded > 0 || self.stored_into > 0 {
            return Role::Handle;
        }
        if self.arithmetic > 0 {
            return Role::Scalar;
        }
        Role::Unknown
    }

    /// A short account of what the code did, for a reader who wants to judge
    /// the classification rather than take it.
    pub fn evidence(&self) -> String {
        let mut parts = Vec::new();
        if let Some(width) = self.load_width {
            parts.push(format!("loaded {width}B"));
        }
        if let Some(width) = self.store_width {
            parts.push(format!("stored {width}B"));
        }
        if self.comparisons > 0 {
            parts.push(format!("compared x{}", self.comparisons));
        }
        if self.forwarded > 0 {
            parts.push(format!("forwarded x{}", self.forwarded));
        }
        if self.stored_into > 0 {
            parts.push(format!("stored as a value x{}", self.stored_into));
        }
        if self.arithmetic > 0 {
            parts.push(format!("arithmetic x{}", self.arithmetic));
        }
        if parts.is_empty() {
            "unused in the scanned body".to_owned()
        } else {
            parts.join(", ")
        }
    }
}

/// The inferred shape of one function.
#[derive(Debug, Clone)]
pub struct FunctionAbi {
    /// The function's name, where the module kept one.
    pub name: String,
    /// Index in the module's function index space.
    pub index: u32,
    /// One entry per declared parameter, in order.
    pub params: Vec<ParamEvidence>,
    /// Instructions scanned. A very small body usually means the real work is
    /// behind a call, and the evidence is correspondingly thin.
    pub scanned: usize,
    /// Table slots holding this function, if any.
    ///
    /// A function nothing calls directly is not dead: it is a callback, and it
    /// runs only once something stores its slot. Knowing the slot is what turns
    /// "unreachable" into "reached when this vtable entry is filled in".
    pub table_slots: Vec<u32>,
    /// The instructions themselves, when the body is short enough to read.
    ///
    /// A trampoline is a handful of instructions that loads a function pointer
    /// and jumps through it, and no amount of parameter classification conveys
    /// that as well as seeing it.
    pub body: Vec<String>,
}

impl FunctionAbi {
    /// Renders the signature with each parameter's inferred role.
    pub fn describe(&self) -> String {
        let mut out = format!("{} (function #{})\n", self.name, self.index);
        for param in &self.params {
            out.push_str(&format!(
                "  arg{:<2} {:<34} {}\n",
                param.index,
                param.role().to_string(),
                param.evidence()
            ));
        }
        if !self.table_slots.is_empty() {
            out.push_str(&format!(
                "  in function table at slot{} {:?}\n",
                if self.table_slots.len() == 1 { "" } else { "s" },
                self.table_slots
            ));
        }
        out.push_str(&format!("  ({} instructions scanned)\n", self.scanned));
        if !self.body.is_empty() {
            let shown = self.body.len();
            if shown < self.scanned {
                out.push_str(&format!(
                    "  body (first {shown} of {}; raise --body for more):\n",
                    self.scanned
                ));
            } else {
                out.push_str("  body:\n");
            }
            for line in &self.body {
                out.push_str(&format!("    {line}\n"));
            }
        }
        out
    }

    /// Whether this looks like a trampoline: a few instructions that load a
    /// pointer out of the first argument and call through it.
    ///
    /// Worth naming, because it changes what the caller has to do — the
    /// argument layout that matters belongs to the callee, and the first
    /// argument has to be an object whose function pointer is already valid.
    pub fn is_trampoline(&self) -> bool {
        const SHORT: usize = 24;

        self.scanned <= SHORT
            && self
                .params
                .first()
                .is_some_and(|param| param.load_width.is_some())
            && self
                .body
                .iter()
                .any(|line| line.contains("call_indirect") || line.contains("call "))
    }
}

/// What an operand on the abstract stack came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// Directly a parameter.
    Param(u32),
    /// Derived from a parameter by arithmetic — still worth tracking, because
    /// `base + offset` is how every array access looks.
    Derived(u32),
    Other,
}

impl Origin {
    fn param(self) -> Option<u32> {
        match self {
            Self::Param(index) | Self::Derived(index) => Some(index),
            Self::Other => None,
        }
    }
}

/// The constant offset an active element segment starts at.
fn const_offset(expr: &wasmparser::ConstExpr<'_>) -> Option<u32> {
    let mut reader = expr.get_operators_reader();
    match reader.read().ok()? {
        Operator::I32Const { value } => Some(value as u32),
        _ => None,
    }
}

/// Infers the ABI of the function a table slot points at.
///
/// This is how a trampoline is followed: `oracle abi` reports the slot an
/// object's vtable holds, and this resolves it to the function that actually
/// takes the arguments.
pub fn infer_table_slot(bytes: &[u8], slot: u32) -> Result<Option<FunctionAbi>> {
    let module = Layout::read(bytes)?;
    let Some(func) = module.table.get(&slot).copied() else {
        return Ok(None);
    };
    module.analyse(bytes, &format!("table[{slot}]"), func)
}

/// Every table slot that points at `func`, in slot order.
///
/// The inverse of [`infer_table_slot`]: resolution answers "which function does
/// this slot call", derivation needs "through which slots is this function
/// reachable" before it can drive it with `Runtime::call_table`.
pub fn table_slots_of(bytes: &[u8], func: u32) -> Result<Vec<u32>> {
    let module = Layout::read(bytes)?;
    Ok(module
        .table
        .iter()
        .filter(|(_, target)| **target == func)
        .map(|(slot, _)| *slot)
        .collect())
}

/// How many functions the module's function space holds, imports included.
///
/// A selector's `index_hint` is checked against this before anything else, so
/// a hint recorded against one capture fails loudly on a shorter one rather
/// than resolving to whatever happens to sit there.
pub fn function_count(bytes: &[u8]) -> Result<usize> {
    let module = Layout::read(bytes)?;
    Ok(module.func_types.len())
}

/// Infers the ABI of one function by its index in the module's function space.
///
/// This is the form a trap needs: a wasm backtrace names the frames by index and
/// nothing else, so `wasm function 151` is otherwise unreadable. The name in the
/// result is whatever the module says — an import's `module.field`, an export's
/// name, or `func[N]` when it is neither.
pub fn infer_index(bytes: &[u8], func: u32) -> Result<Option<FunctionAbi>> {
    let module = Layout::read(bytes)?;
    let name = module.name_of(func);

    // An import has no body to analyse, but naming it is the whole answer:
    // a trap inside `env.abort` says something very different from a trap in
    // guest code.
    if (func as usize) < module.imported_funcs {
        return Ok(Some(FunctionAbi {
            name,
            index: func,
            params: Vec::new(),
            scanned: 0,
            body: Vec::new(),
            table_slots: Vec::new(),
        }));
    }

    module.analyse(bytes, &name, func)
}

/// Infers the ABI of every exported function whose name matches `filter`.
pub fn infer(bytes: &[u8], filter: impl Fn(&str) -> bool) -> Result<Vec<FunctionAbi>> {
    let module = Layout::read(bytes)?;

    let mut out = Vec::new();
    for (name, index) in &module.exported_functions {
        if !filter(name) {
            continue;
        }
        let Some(abi) = module.analyse(bytes, name, *index)? else {
            continue;
        };
        out.push(abi);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The parts of a module needed to find and read one function body.
struct Layout {
    /// Type index of every function, imports first.
    func_types: Vec<u32>,
    /// Parameter count per type index.
    type_params: Vec<usize>,
    /// Functions defined in this module, not imported.
    imported_funcs: usize,
    /// `module.field` for each imported function, in function-index order.
    import_names: Vec<String>,
    exported_functions: Vec<(String, u32)>,
    /// Function index for each table slot, from the element segments.
    ///
    /// A trampoline calls through a slot it read out of an object, so
    /// following one means resolving that slot to a function.
    table: BTreeMap<u32, u32>,
    /// Active data segments as (base address, contents).
    ///
    /// Carried so that a constant appearing in a body can be shown as the text
    /// it addresses. On a minified module that is often the only readable thing
    /// left: `call 53` says nothing, but the string it is handed says
    /// "Not a JPEG file".
    data: Vec<(u32, Vec<u8>)>,
}

impl Layout {
    fn read(bytes: &[u8]) -> Result<Self> {
        let mut type_params = Vec::new();
        let mut func_types = Vec::new();
        let mut imported_funcs = 0;
        let mut import_names = Vec::new();
        let mut exported_functions = Vec::new();
        let mut table = BTreeMap::new();
        let mut data = Vec::new();
        let mut passive: BTreeMap<u32, Vec<u8>> = BTreeMap::new();

        for payload in Parser::new(0).parse_all(bytes) {
            match payload.context("parsing module")? {
                Payload::TypeSection(reader) => {
                    for group in reader {
                        for sub in group.context("type")?.into_types() {
                            let count = match &sub.composite_type.inner {
                                wasmparser::CompositeInnerType::Func(func) => func.params().len(),
                                _ => 0,
                            };
                            type_params.push(count);
                        }
                    }
                }
                Payload::ImportSection(reader) => {
                    for import in reader.into_imports() {
                        let import = import.context("import")?;
                        if let wasmparser::TypeRef::Func(ty) | wasmparser::TypeRef::FuncExact(ty) =
                            import.ty
                        {
                            func_types.push(ty);
                            imported_funcs += 1;
                            import_names.push(format!("{}.{}", import.module, import.name));
                        }
                    }
                }
                Payload::FunctionSection(reader) => {
                    for ty in reader {
                        func_types.push(ty.context("function")?);
                    }
                }
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.context("export")?;
                        if export.kind == wasmparser::ExternalKind::Func {
                            exported_functions.push((export.name.to_owned(), export.index));
                        }
                    }
                }
                Payload::ElementSection(reader) => {
                    for element in reader {
                        let element = element.context("element segment")?;
                        // Only active segments with a constant offset place
                        // functions at known slots.
                        let wasmparser::ElementKind::Active { offset_expr, .. } = element.kind
                        else {
                            continue;
                        };
                        let Some(base) = const_offset(&offset_expr) else {
                            continue;
                        };
                        if let wasmparser::ElementItems::Functions(items) = element.items {
                            for (slot, func) in items.into_iter().enumerate() {
                                table.insert(base + slot as u32, func.context("element")?);
                            }
                        }
                    }
                }
                Payload::DataSection(reader) => {
                    for (index, segment) in reader.into_iter().enumerate() {
                        let segment = segment.context("data segment")?;
                        match segment.kind {
                            wasmparser::DataKind::Active { offset_expr, .. } => {
                                if let Some(base) = const_offset(&offset_expr) {
                                    data.push((base, segment.data.to_vec()));
                                }
                            }
                            // A passive segment carries no address: the module
                            // copies it in with `memory.init`, and where it
                            // lands is only visible in that instruction. Held by
                            // index until the code section resolves it below.
                            wasmparser::DataKind::Passive => {
                                passive.insert(index as u32, segment.data.to_vec());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Every module with shared memory — anything with threads — has only
        // passive segments, so without this no string in it can be read.
        data.extend(resolve_passive_segments(bytes, &passive)?);

        Ok(Self {
            func_types,
            type_params,
            imported_funcs,
            import_names,
            data,
            exported_functions,
            table,
        })
    }

    /// The best name the module offers for a function index.
    fn name_of(&self, func: u32) -> String {
        if let Some((name, _)) = self
            .exported_functions
            .iter()
            .find(|(_, index)| *index == func)
        {
            return name.clone();
        }
        if let Some(name) = self.import_names.get(func as usize) {
            return name.clone();
        }
        format!("func[{func}]")
    }

    fn param_count(&self, func: u32) -> usize {
        self.func_types
            .get(func as usize)
            .and_then(|ty| self.type_params.get(*ty as usize))
            .copied()
            .unwrap_or(0)
    }

    /// Reads the body of `func` and classifies its parameters.
    fn analyse(&self, bytes: &[u8], name: &str, func: u32) -> Result<Option<FunctionAbi>> {
        // Imported functions have no body here.
        let Some(local_index) = (func as usize).checked_sub(self.imported_funcs) else {
            return Ok(None);
        };
        // A function taking no parameters still gets scanned: there is nothing
        // to classify, but the listing is the point when the caller is reading
        // a trap frame rather than looking for an argument layout.
        let params = self.param_count(func);

        let mut seen = 0usize;
        for payload in Parser::new(0).parse_all(bytes) {
            let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
                continue;
            };
            if seen != local_index {
                seen += 1;
                continue;
            }
            let mut abi = scan_body(name, func, params, &body, &self.data)?;
            abi.table_slots = self
                .table
                .iter()
                .filter(|(_, target)| **target == func)
                .map(|(slot, _)| *slot)
                .collect();
            return Ok(Some(abi));
        }
        Err(anyhow!("no body for function #{func}"))
    }
}

/// Walks one function body, tracking where each operand came from.
/// Longest body recorded instruction-by-instruction by default. A caller
/// chasing indirection can ask for more.
const BODY_LISTING_LIMIT: usize = 24;

/// How many instructions to record. Set by `infer_with_body`.
static BODY_LIMIT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(BODY_LISTING_LIMIT);

fn body_limit() -> usize {
    BODY_LIMIT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Records up to `limit` instructions per function for the duration of the
/// call. Following a chain of trampolines means reading real bodies, not just
/// the handful that fit a summary.
pub fn set_body_limit(limit: usize) {
    BODY_LIMIT.store(limit, std::sync::atomic::Ordering::Relaxed);
}

/// Longest run of static text quoted next to a constant.
///
/// Long enough for a log line, which is what these mostly are: cutting one in
/// half hides the condition it reports, which is the only reason to read it.
const TEXT_PREVIEW: usize = 160;

/// The text a constant addresses, if it addresses readable text at all.
///
/// Rust and C++ both keep their panic and error messages as unterminated slices
/// in a data segment, so this stops at the first byte that is not printable
/// rather than at a NUL.
fn static_text(data: &[(u32, Vec<u8>)], addr: u32) -> Option<String> {
    /// Below this a "string" is as likely to be two bytes of a struct that
    /// happen to be printable.
    const MIN_RUN: usize = 4;

    let (base, bytes) = data
        .iter()
        .find(|(base, bytes)| addr >= *base && ((addr - base) as usize) < bytes.len())?;
    let start = (addr - base) as usize;

    let text: String = bytes[start..]
        .iter()
        .take(TEXT_PREVIEW)
        .take_while(|byte| matches!(byte, 0x20..=0x7e))
        .map(|byte| *byte as char)
        .collect();

    (text.len() >= MIN_RUN).then_some(text)
}

/// The text at a static address, for naming what a call site is handed.
///
/// A minified module still keeps its `__FILE__` strings and function names in
/// data segments, and a logging call is passed both as plain constants. That
/// makes this the shortest route from a bare function index to the C++ name it
/// was compiled from — reading the constants pushed before its `call` to the
/// logger. Resolving those by hand means re-deriving segment bases and getting
/// them subtly wrong; this reuses the module's own reader instead.
pub fn static_string(bytes: &[u8], addr: u32) -> Result<Option<String>> {
    let module = Layout::read(bytes)?;
    Ok(static_text(&module.data, addr))
}

fn scan_body(
    name: &str,
    index: u32,
    params: usize,
    body: &wasmparser::FunctionBody<'_>,
    data: &[(u32, Vec<u8>)],
) -> Result<FunctionAbi> {
    let mut evidence: BTreeMap<u32, ParamEvidence> = (0..params as u32)
        .map(|i| {
            (
                i,
                ParamEvidence {
                    index: i,
                    ..ParamEvidence::default()
                },
            )
        })
        .collect();

    /// Drops a parameter from the live set unless the value written back is the
    /// parameter itself.
    ///
    /// `x = round_up(x)` — an allocator aligning its size — keeps the slot
    /// meaning what it meant. `x = obj->field` does not: the slot is now a
    /// temporary, and everything after it is evidence about that temporary.
    fn retire_if_clobbered(
        live: &mut std::collections::BTreeSet<u32>,
        local_index: u32,
        value: Origin,
    ) {
        if value.param() != Some(local_index) {
            live.remove(&local_index);
        }
    }

    // Parameters whose slot still holds the incoming argument.
    //
    // wasm locals are not SSA: an optimiser reuses a parameter's slot as a
    // temporary the moment the argument is dead. Everything after that store is
    // evidence about the temporary, and crediting it to the parameter is how a
    // caller-supplied length ends up labelled a pointer. Being linear, this
    // retires a slot on the first store even if only one branch reaches it — it
    // gives up evidence rather than inventing it.
    let mut live: std::collections::BTreeSet<u32> = (0..params as u32).collect();

    let mut stack: Vec<Origin> = Vec::new();
    let mut scanned = 0usize;
    let mut listing: Vec<String> = Vec::new();

    let record = |evidence: &mut BTreeMap<u32, ParamEvidence>,
                  origin: Origin,
                  apply: &dyn Fn(&mut ParamEvidence)| {
        if let Some(param) = origin.param()
            && let Some(entry) = evidence.get_mut(&param)
        {
            apply(entry);
        }
    };

    for op in body.get_operators_reader()? {
        let op = op.context("reading instruction")?;
        scanned += 1;
        if listing.len() < body_limit() {
            let mut line = render(&op);
            // A constant that addresses text is almost always the interesting
            // half of the instruction pair that reports an error.
            if let Operator::I32Const { value } = op
                && let Some(text) = static_text(data, value as u32)
            {
                line.push_str(&format!("  ; {text:?}"));
            }
            listing.push(line);
            trim_quoted_text_to_length(&mut listing);
        }

        match op {
            Operator::LocalGet { local_index } => {
                stack.push(if live.contains(&local_index) {
                    Origin::Param(local_index)
                } else {
                    Origin::Other
                });
            }
            Operator::LocalSet { local_index } => {
                let value = stack.pop().unwrap_or(Origin::Other);
                retire_if_clobbered(&mut live, local_index, value);
            }
            Operator::Drop => {
                stack.pop();
            }
            Operator::LocalTee { local_index } => {
                let value = stack.last().copied().unwrap_or(Origin::Other);
                retire_if_clobbered(&mut live, local_index, value);
            }

            // Loads: the address is on top, and the width tells us what it
            // points at.
            Operator::I32Load { .. } | Operator::F32Load { .. } => {
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.load_width = e.load_width.max(Some(4))
                });
                stack.push(Origin::Other);
            }
            Operator::I64Load { .. } | Operator::F64Load { .. } => {
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.load_width = e.load_width.max(Some(8))
                });
                stack.push(Origin::Other);
            }
            Operator::I32Load8U { .. }
            | Operator::I32Load8S { .. }
            | Operator::I64Load8U { .. }
            | Operator::I64Load8S { .. } => {
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.load_width = e.load_width.max(Some(1))
                });
                stack.push(Origin::Other);
            }
            Operator::I32Load16U { .. }
            | Operator::I32Load16S { .. }
            | Operator::I64Load16U { .. }
            | Operator::I64Load16S { .. } => {
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.load_width = e.load_width.max(Some(2))
                });
                stack.push(Origin::Other);
            }

            // Stores: value on top, address below it.
            Operator::I32Store { .. } | Operator::F32Store { .. } => {
                let value = stack.pop().unwrap_or(Origin::Other);
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.store_width = e.store_width.max(Some(4))
                });
                record(&mut evidence, value, &|e| e.stored_into += 1);
            }
            Operator::I64Store { .. } | Operator::F64Store { .. } => {
                let value = stack.pop().unwrap_or(Origin::Other);
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.store_width = e.store_width.max(Some(8))
                });
                record(&mut evidence, value, &|e| e.stored_into += 1);
            }
            Operator::I32Store8 { .. } | Operator::I64Store8 { .. } => {
                let value = stack.pop().unwrap_or(Origin::Other);
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.store_width = e.store_width.max(Some(1))
                });
                record(&mut evidence, value, &|e| e.stored_into += 1);
            }
            Operator::I32Store16 { .. } | Operator::I64Store16 { .. } => {
                let value = stack.pop().unwrap_or(Origin::Other);
                let addr = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, addr, &|e| {
                    e.store_width = e.store_width.max(Some(2))
                });
                record(&mut evidence, value, &|e| e.stored_into += 1);
            }

            // Comparisons: what bounds a loop looks like.
            Operator::I32Eq
            | Operator::I32Ne
            | Operator::I32LtS
            | Operator::I32LtU
            | Operator::I32GtS
            | Operator::I32GtU
            | Operator::I32LeS
            | Operator::I32LeU
            | Operator::I32GeS
            | Operator::I32GeU => {
                let right = stack.pop().unwrap_or(Origin::Other);
                let left = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, left, &|e| e.comparisons += 1);
                record(&mut evidence, right, &|e| e.comparisons += 1);
                stack.push(Origin::Other);
            }
            Operator::I32Eqz => {
                let value = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, value, &|e| e.comparisons += 1);
                stack.push(Origin::Other);
            }

            // Arithmetic: `base + offset` keeps the parameter's identity, since
            // that is what indexing into a buffer looks like.
            Operator::I32Add | Operator::I32Sub | Operator::I32Mul | Operator::I32Shl => {
                let right = stack.pop().unwrap_or(Origin::Other);
                let left = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, left, &|e| e.arithmetic += 1);
                record(&mut evidence, right, &|e| e.arithmetic += 1);
                let carried = left.param().or_else(|| right.param());
                stack.push(match carried {
                    Some(param) => Origin::Derived(param),
                    None => Origin::Other,
                });
            }

            Operator::Call { .. } | Operator::CallIndirect { .. } => {
                // The callee's arity is not tracked, so credit whatever
                // parameter-derived operands are on the stack and clear it: a
                // conservative reading that never invents a use.
                for origin in std::mem::take(&mut stack) {
                    record(&mut evidence, origin, &|e| e.forwarded += 1);
                }
            }

            // Producers: they push without consuming. The catch-all below pops
            // one operand, which on a constant would discard the `local.get`
            // that just pushed the parameter — the comparison that follows then
            // sees an empty stack and the parameter looks unused. Nearly every
            // comparison has a constant on one side, so this is most of them.
            Operator::I32Const { .. }
            | Operator::I64Const { .. }
            | Operator::F32Const { .. }
            | Operator::F64Const { .. }
            | Operator::GlobalGet { .. }
            | Operator::MemorySize { .. } => stack.push(Origin::Other),

            // Structured control flow does not consume operands, and clearing
            // the stack here loses every parameter used inside a branch — which
            // is most of them in any function with a loop.
            Operator::Block { .. }
            | Operator::Loop { .. }
            | Operator::Else
            | Operator::End
            | Operator::Return
            | Operator::Nop => {}

            // These consume a condition but leave the rest in place.
            Operator::If { .. } | Operator::BrIf { .. } | Operator::BrTable { .. } => {
                let condition = stack.pop().unwrap_or(Origin::Other);
                record(&mut evidence, condition, &|e| e.comparisons += 1);
            }
            Operator::Br { .. } => {}

            Operator::Select => {
                stack.pop();
                stack.pop();
                stack.pop();
                stack.push(Origin::Other);
            }

            _ => {
                // Anything else: drop one operand rather than the whole stack.
                // Clearing was losing the operands of everything that followed.
                stack.pop();
                stack.push(Origin::Other);
            }
        }
    }

    Ok(FunctionAbi {
        name: name.to_owned(),
        index,
        params: evidence.into_values().collect(),
        scanned,
        // Filled in by the caller, which is what knows the element section.
        table_slots: Vec::new(),
        // The listing is already capped at the limit; showing the first N
        // instructions of a long function is what makes a body explorable at
        // all, and `scanned` next to it says how much was left out.
        body: listing,
    })
}

/// A readable form of one instruction. Only the operands that matter for
/// following data through a function are shown.
fn render(op: &Operator<'_>) -> String {
    match op {
        Operator::LocalGet { local_index } => format!("local.get {local_index}"),
        Operator::LocalSet { local_index } => format!("local.set {local_index}"),
        Operator::LocalTee { local_index } => format!("local.tee {local_index}"),
        Operator::GlobalGet { global_index } => format!("global.get {global_index}"),
        Operator::GlobalSet { global_index } => format!("global.set {global_index}"),
        Operator::I32Const { value } => format!("i32.const {value}"),
        Operator::I64Const { value } => format!("i64.const {value}"),
        Operator::I32Load { memarg } => format!("i32.load offset={}", memarg.offset),
        Operator::I32Load8U { memarg } => format!("i32.load8_u offset={}", memarg.offset),
        Operator::I32Store { memarg } => format!("i32.store offset={}", memarg.offset),
        Operator::Call { function_index } => format!("call {function_index}"),
        Operator::CallIndirect { type_index, .. } => format!("call_indirect type={type_index}"),
        Operator::ReturnCall { function_index } => format!("return_call {function_index}"),
        Operator::ReturnCallIndirect { type_index, .. } => {
            format!("return_call_indirect type={type_index}")
        }
        // The depth is the whole meaning of a branch: `br_if 0` continues a
        // loop and `br_if 1` leaves the block around it, and without it a
        // listing reads as if control fell through when it did not.
        Operator::Br { relative_depth } => format!("br {relative_depth}"),
        // The targets are the whole content of a `br_table`: without them a
        // switch reads as an opaque jump, and matching cases to bodies by the
        // order they appear in the listing is guesswork — the labels are what
        // actually says which case goes where.
        Operator::BrTable { targets } => {
            let mut labels: Vec<String> = targets
                .targets()
                .map(|target| match target {
                    Ok(depth) => depth.to_string(),
                    Err(_) => "?".to_owned(),
                })
                .collect();
            labels.push(format!("default {}", targets.default()));
            format!("br_table [{}]", labels.join(" "))
        }
        Operator::BrIf { relative_depth } => format!("br_if {relative_depth}"),
        Operator::End => "end".to_owned(),
        Operator::Return => "return".to_owned(),
        other => {
            let text = format!("{other:?}");
            let name = text
                .split_whitespace()
                .next()
                .unwrap_or("?")
                .trim_end_matches('{')
                .to_lowercase();

            // Every remaining memory operator — `i64.store`, the narrow widths,
            // the SIMD lane accesses — carries a `MemArg`, and its offset is the
            // struct field being touched. Pulling it out of the debug rendering
            // covers all of them without enumerating fifty variants, and an
            // omitted offset reads as zero, which is a different instruction.
            match field_after(&text, "offset: ") {
                Some(offset) => format!("{name} offset={offset}"),
                None => name,
            }
        }
    }
}

/// Cuts a quoted string down to the length the next instruction gives.
///
/// Rust and C++ keep string data as unterminated slices, so reading from an
/// address runs straight into whatever sits after it — `"…on a `None`
/// valuepanicked at "` is two separate messages. The pointer is almost always
/// followed by its own length, and using it turns a smear back into the message
/// the code is actually reporting.
fn trim_quoted_text_to_length(listing: &mut [String]) {
    const MARKER: &str = "  ; \"";

    let [.., quoted, next] = listing else {
        return;
    };
    let Some(length) = next.strip_prefix("i32.const ").and_then(|n| n.parse().ok()) else {
        return;
    };
    let Some(start) = quoted.find(MARKER) else {
        return;
    };

    // The quoted run is at most TEXT_PREVIEW long; a length beyond it is not
    // describing this string, and one at or past the run leaves it unchanged.
    let text = &quoted[start + MARKER.len()..quoted.len() - 1];
    if length == 0 || length >= text.len() {
        return;
    }
    let trimmed = format!("{}{MARKER}{}…\"", &quoted[..start], &text[..length]);
    *quoted = trimmed;
}

/// The integer following `key` in a debug rendering.
fn field_after(text: &str, key: &str) -> Option<u64> {
    let rest = text.split_once(key)?.1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Finds the imports that are emscripten `invoke_*` trampolines, by shape.
///
/// On an unminified module the name gives it away. On a minified one every
/// import is a single letter, and stubbing a trampoline is not neutral: the
/// call it should have dispatched silently does not happen, and the guest reads
/// back a result that was never produced. That is how the mozjpeg module got as
/// far as building a compress struct and then reporting failure — its
/// `jpeg_start_compress` was never called.
///
/// The generated code is unmistakable, though. Emscripten clears a fixed
/// `__THREW__` word, makes the call, and reads the word back:
///
/// ```text
///   i32.const 224072 / i32.const 0 / i32.store    ; __THREW__ = 0
///   <args> / call N                                ; the trampoline
///   i32.const 224072 / i32.load                    ; did it throw?
/// ```
///
/// Both halves must name the same address, which is what keeps an ordinary
/// import that happens to precede a global read from matching.
///
/// Returns the function index of each trampoline, paired with the address it
/// flags through.
pub fn find_invoke_imports(bytes: &[u8]) -> Result<BTreeMap<u32, u32>> {
    /// How far back the clearing store may sit. Only the callee's arguments come
    /// between, and those are few.
    const LOOKBEHIND: usize = 24;
    /// The read-back follows the call immediately, allowing for the constant.
    const LOOKAHEAD: usize = 3;

    let module = Layout::read(bytes)?;
    let mut found = BTreeMap::new();

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };

        // (address, position) of the most recent `__THREW__ = 0`.
        let mut cleared: Option<(u32, usize)> = None;
        // A call awaiting its read-back: (import index, address, position).
        let mut pending: Option<(u32, u32, usize)> = None;
        let mut window: Vec<Operator<'_>> = Vec::new();

        for (position, op) in body.get_operators_reader()?.into_iter().enumerate() {
            let op = op.context("reading instruction")?;

            // `i32.const A / i32.const 0 / i32.store`
            if let Operator::I32Store { .. } = op
                && let [
                    ..,
                    Operator::I32Const { value: addr },
                    Operator::I32Const { value: 0 },
                ] = window.as_slice()
            {
                cleared = Some((*addr as u32, position));
            }

            // `i32.const A / i32.load` after a call to that same address.
            if let Operator::I32Load { .. } = op
                && let [.., Operator::I32Const { value: addr }] = window.as_slice()
                && let Some((func, expected, at)) = pending
                && *addr as u32 == expected
                && position - at <= LOOKAHEAD
            {
                found.insert(func, expected);
                pending = None;
            }

            if let Operator::Call { function_index } = op
                && (function_index as usize) < module.imported_funcs
                && let Some((addr, at)) = cleared
                && position - at <= LOOKBEHIND
            {
                pending = Some((function_index, addr, position));
            }

            window.push(op);
            if window.len() > 3 {
                window.remove(0);
            }
        }
    }

    Ok(found)
}

/// Places passive data segments at the addresses `memory.init` copies them to.
///
/// A module with shared memory cannot use active segments: the memory is
/// initialised once, by whichever thread gets there first, rather than per
/// instance. Emscripten therefore emits everything passive and copies it in
/// from a start function, which means the address of every string lives in the
/// code rather than in the data section.
///
/// The generated sequence is fixed:
///
/// ```text
///   i32.const <dest>  / i32.const <offset> / i32.const <len> / memory.init <n>
/// ```
///
/// Reading it back is what makes `static_text` work on a threaded module, and
/// so what makes a minified threaded module readable at all.
fn resolve_passive_segments(
    bytes: &[u8],
    passive: &BTreeMap<u32, Vec<u8>>,
) -> Result<Vec<(u32, Vec<u8>)>> {
    if passive.is_empty() {
        return Ok(Vec::new());
    }

    let mut placed = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };

        let mut constants: Vec<u32> = Vec::new();
        for op in body.get_operators_reader()? {
            match op.context("reading instruction")? {
                Operator::I32Const { value } => constants.push(value as u32),
                Operator::MemoryInit { data_index, .. } => {
                    // The three constants immediately before the copy.
                    if let [.., dest, offset, len] = constants.as_slice()
                        && let Some(segment) = passive.get(&data_index)
                    {
                        let (start, end) = (*offset as usize, (*offset + *len) as usize);
                        if start <= end && end <= segment.len() {
                            placed.push((*dest, segment[start..end].to_vec()));
                        }
                    }
                    constants.clear();
                }
                // Anything else invalidates the run: the constants a copy uses
                // are the three directly in front of it.
                _ => constants.clear(),
            }
        }
    }

    Ok(placed)
}

/// A string in the module's data, and the functions that mention its address.
#[derive(Debug, Clone)]
pub struct StringRef {
    /// Address of the string in linear memory.
    pub address: u32,
    /// The string itself.
    pub text: String,
    /// Indices of functions that load this address as a constant.
    pub referenced_by: Vec<u32>,
    /// Addresses in data holding a pointer to this string.
    ///
    /// A message reached through a table — an enum's names, a dispatch array —
    /// is never named by any instruction. The pointer is the only link, and
    /// following it is the difference between finding the emitting code and
    /// concluding the string is dead.
    pub pointer_slots: Vec<u32>,
}

/// Finds the code that reaches for a given string.
///
/// A log line is the most legible thing a stripped module produces, and it
/// names the exact branch that produced it. Going from the message back to the
/// code that emits it is otherwise the hard direction: the message lives in a
/// data segment and the only link is a bare `i32.const`.
///
/// Matching is by substring, so a partial message is enough.
pub fn find_string_refs(bytes: &[u8], needle: &str) -> Result<Vec<StringRef>> {
    let module = Layout::read(bytes)?;

    // Where each match sits, keyed by address so several hits in one segment
    // stay distinct.
    let mut wanted: BTreeMap<u32, String> = BTreeMap::new();
    // Searched over the raw bytes: `from_utf8_lossy` replaces each invalid byte
    // with a three-byte U+FFFD, which shifts every offset after it and would
    // point the result at the wrong string.
    let pattern = needle.as_bytes();
    for (base, contents) in &module.data {
        let offsets = contents
            .windows(pattern.len())
            .enumerate()
            .filter(|(_, window)| *window == pattern)
            .map(|(offset, _)| offset);
        for offset in offsets {
            // Back up to where the string actually starts. The linker merges a
            // string that is the suffix of another, so a match in the middle
            // has no reference of its own — only the containing string does,
            // and reporting the match offset makes a live message look dead.
            let start = contents[..offset]
                .iter()
                .rposition(|byte| !matches!(byte, 0x20..=0x7e))
                .map_or(0, |position| position + 1);

            let text: String = contents[start..]
                .iter()
                .take(TEXT_PREVIEW)
                .take_while(|byte| matches!(byte, 0x20..=0x7e))
                .map(|byte| *byte as char)
                .collect();
            wanted.insert(base + start as u32, text);
        }
    }
    if wanted.is_empty() {
        return Ok(Vec::new());
    }

    let mut refs: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut local_index = 0usize;
    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };
        let func = (module.imported_funcs + local_index) as u32;
        local_index += 1;

        for op in body.get_operators_reader()? {
            if let Operator::I32Const { value } = op.context("reading instruction")?
                && wanted.contains_key(&(value as u32))
            {
                let entry = refs.entry(value as u32).or_default();
                if entry.last() != Some(&func) {
                    entry.push(func);
                }
            }
        }
    }

    Ok(wanted
        .into_iter()
        .map(|address_and_text| {
            let (address, text) = address_and_text;
            let referenced_by = refs.remove(&address).unwrap_or_default();
            // Only worth the scan when nothing names the string directly.
            let pointer_slots = if referenced_by.is_empty() {
                find_pointer_slots(&module.data, address)
            } else {
                Vec::new()
            };
            StringRef {
                address,
                text,
                referenced_by,
                pointer_slots,
            }
        })
        .collect())
}

/// Addresses in data that hold `target` as a little-endian pointer.
fn find_pointer_slots(data: &[(u32, Vec<u8>)], target: u32) -> Vec<u32> {
    let needle = target.to_le_bytes();
    let mut slots = Vec::new();

    for (base, contents) in data {
        // Pointers in a table are word-aligned, so only aligned positions are
        // candidates; an unaligned hit is a coincidence in unrelated bytes.
        let start = base.next_multiple_of(4) - base;
        for offset in (start as usize..contents.len().saturating_sub(3)).step_by(4) {
            if contents[offset..offset + 4] == needle {
                slots.push(base + offset as u32);
            }
        }
    }
    slots
}

/// A function that loads an address, and how far into a table it points.
#[derive(Debug, Clone)]
pub struct AddressRef {
    /// Index of the function that reaches it.
    pub function: u32,
    /// The constant the function actually loads.
    pub loaded: u32,
    /// `address - loaded`: zero when the function names the address outright,
    /// and otherwise the byte offset into the table it holds the base of.
    pub offset: u32,
}

/// Finds the code that reaches an address, directly or as a table base.
///
/// A message reached through an array of pointers is never named by any
/// instruction; what the code holds is the base of the array, and the message
/// is at `base + 4 * index`. Searching a window back from the address finds
/// that base, which is the function actually doing the work.
///
/// `window` bounds how far back a base may sit. Too wide and unrelated
/// constants match, so the caller chooses it against the table it expects.
pub fn find_address_refs(bytes: &[u8], address: u32, window: u32) -> Result<Vec<AddressRef>> {
    let module = Layout::read(bytes)?;
    let low = address.saturating_sub(window);

    let mut found: Vec<AddressRef> = Vec::new();
    let mut local_index = 0usize;

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };
        let function = (module.imported_funcs + local_index) as u32;
        local_index += 1;

        for op in body.get_operators_reader()? {
            let Operator::I32Const { value } = op.context("reading instruction")? else {
                continue;
            };
            let loaded = value as u32;
            // Word-aligned only: a table of pointers is indexed by whole words,
            // so a base that does not divide evenly is not this table's.
            if loaded < low || loaded > address || !(address - loaded).is_multiple_of(4) {
                continue;
            }
            let candidate = AddressRef {
                function,
                loaded,
                offset: address - loaded,
            };
            if !found
                .iter()
                .any(|seen| seen.function == function && seen.loaded == loaded)
            {
                found.push(candidate);
            }
        }
    }

    // Nearest base first: the closest constant is the likeliest table start.
    found.sort_by_key(|entry| entry.offset);
    Ok(found)
}

/// Finds every function that calls `callee` directly.
///
/// The reverse direction of a backtrace. A stripped module gives no call graph,
/// and the function that *logs* something is rarely the one that decided to:
/// getting from the message to the decision means walking back up, one caller
/// at a time.
///
/// Indirect calls are not followed — `call_indirect` names a type, not a
/// target. `infer_table_slot` covers that direction.
pub fn find_callers(bytes: &[u8], callee: u32) -> Result<Vec<u32>> {
    let module = Layout::read(bytes)?;

    let mut callers = Vec::new();
    let mut local_index = 0usize;

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };
        let function = (module.imported_funcs + local_index) as u32;
        local_index += 1;

        for op in body.get_operators_reader()? {
            match op.context("reading instruction")? {
                Operator::Call { function_index } | Operator::ReturnCall { function_index }
                    if function_index == callee =>
                {
                    callers.push(function);
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(callers)
}

/// Reads a table of string pointers as the named enum it usually is.
///
/// `find_string_refs` gets from a message to the slot holding it; this goes the
/// other way, turning the slot's neighbourhood into the list it belongs to. An
/// enum's name table is laid out in declaration order, so the index of an entry
/// *is* the enum value — which is how a bare `call_term_reason: 27` in a log
/// becomes a name.
///
/// Entries that do not address readable text come back as `None`, which is what
/// bounds the guess: a run of them means the table ended.
pub fn read_string_table(bytes: &[u8], base: u32, count: u32) -> Result<Vec<Option<String>>> {
    let module = Layout::read(bytes)?;

    let word_at = |addr: u32| -> Option<u32> {
        let (segment_base, contents) = module.data.iter().find(|(segment_base, contents)| {
            addr >= *segment_base && ((addr - segment_base) as usize) + 4 <= contents.len()
        })?;
        let at = (addr - segment_base) as usize;
        Some(u32::from_le_bytes(contents[at..at + 4].try_into().ok()?))
    };

    Ok((0..count)
        .map(|index| {
            let pointer = word_at(base + index * 4)?;
            static_text(&module.data, pointer)
        })
        .collect())
}

/// Finds every function that loads a given constant.
///
/// A callback reaches the guest as a table index, and the index is an ordinary
/// `i32.const`. Nothing references it as an address and nothing calls it
/// directly, so neither `find_address_refs` nor `find_callers` sees it: the
/// only trace is the constant itself. This is how a slot is traced back to
/// whatever stores or dispatches it.
///
/// Small values match a lot of unrelated arithmetic, which is why the result
/// carries how many times each function used it — one hit in a large function
/// is usually noise, and the caller can judge.
pub fn find_constant_users(bytes: &[u8], value: i32) -> Result<Vec<(u32, usize)>> {
    let module = Layout::read(bytes)?;

    let mut found = Vec::new();
    let mut local_index = 0usize;

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::CodeSectionEntry(body) = payload.context("parsing module")? else {
            continue;
        };
        let function = (module.imported_funcs + local_index) as u32;
        local_index += 1;

        let mut uses = 0usize;
        for op in body.get_operators_reader()? {
            if let Operator::I32Const { value: seen } = op.context("reading instruction")?
                && seen == value
            {
                uses += 1;
            }
        }
        if uses > 0 {
            found.push((function, uses));
        }
    }

    Ok(found)
}
