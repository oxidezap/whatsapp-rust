//! Deterministic derivation of vectors from a captured module.
//!
//! `oracle derive --spec spec.json --out dir/` runs a small program against
//! the pinned bytes of one capture and writes outputs plus a manifest that
//! records everything needed to reproduce them: the module's hash, the
//! resolved function indices and table slots, and every output's hash.
//!
//! The spec never names a raw index as truth. Each function is a *selector*
//! — an `index_hint` settled by independent facts (a string the function must
//! reference, an expected body fingerprint). A capture bump renumbers every
//! function, so a hint that silently answers about different code is the
//! failure this exists to prevent: resolution refuses rather than guesses.
//!
//! Execution is single-threaded on purpose. Threads make runs irreproducible,
//! and everything derivable so far — static tables, leaf DSP over scratch
//! buffers — needs none.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::Val;

/// Pins the exact bytes a derivation runs against.
///
/// Hash before use, like `cargo xt oracle fetch`: an oracle that quietly
/// answers from a different build is worse than one that does not run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePin {
    /// Module id, as `Catalog` reports it.
    pub id: String,
    /// Expected SHA-256 of the `.wasm` file, hex.
    pub sha256: String,
    /// Expected size in bytes. Checked first: a truncated download has the
    /// right name and the wrong everything else.
    pub size: u64,
}

/// Selects one function without trusting a recorded index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuncSelector {
    /// Where the function lived in the capture the spec was written against.
    /// A starting point for the checks below, never the answer by itself.
    pub index_hint: u32,
    /// A string the function must reference (substring match on data-segment
    /// strings, via [`crate::abi::find_string_refs`]). When set, the hint is
    /// accepted only if it is among the referencing functions.
    pub must_hold_string: Option<String>,
    /// Expected body fingerprint (see `unwasm_core::analysis::fingerprint`).
    /// When set, a changed body fails resolution instead of running.
    pub expect_fingerprint: Option<u64>,
}

/// Where `derive` resolves a selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolved {
    /// Accepted function index in this module's function space.
    pub index: u32,
    /// Table slots pointing at it, in slot order.
    pub slots: Vec<u32>,
    /// Body fingerprint, when the function is long enough to have one.
    pub fingerprint: Option<u64>,
}

/// An argument to a guest call: `$name` reads a register, otherwise an `i32`
/// literal. Anything else is refused — a malformed value running the guest on
/// input the caller never gave it is the worst available outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    /// `$register`.
    Reg(String),
    /// Integer literal.
    Lit(i32),
    /// An explicitly typed IEEE-754 single-precision argument.
    Float32 {
        /// Value rounded to f32 before entering the guest.
        f32: f32,
    },
}

/// A byte length: literal, or `$register` holding a length the guest just
/// reported (packet sizes, decoded counts). Length-prefixed protocols make
/// literals unknowable upfront; refusing dynamic lengths would cap every
/// derivation at one statically-sized frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Len {
    /// `$register` holding the length.
    Reg(String),
    /// Literal length.
    Lit(u32),
}

/// One step of a derivation program.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    /// Instantiate the pinned module, single-threaded.
    Instantiate,
    /// Run the static constructors (populates embind, static memory).
    RunCtors,
    /// Attach the engine log ring. The module logs through it; without one
    /// the first log call faults on a null ring pointer.
    LogRing {
        /// Ring size in bytes. `1 << 20` matches what the CLI uses.
        len: u32,
    },
    /// Allocate `len` bytes with the guest allocator into a register.
    Malloc {
        /// Register receiving the pointer.
        #[serde(rename = "as")]
        as_reg: String,
        /// Bytes to allocate.
        len: u32,
    },
    /// Write hex bytes at a pointer.
    Write {
        /// Where to write: `$register` or literal address.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Exact bytes, even-length hex.
        hex: String,
    },
    /// Fill memory with one byte value. `malloc` does not zero, and forging a
    /// struct the guest will read means owning every byte of it.
    Fill {
        /// First address: `$register` or literal.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Bytes to fill.
        len: u32,
        /// Fill value.
        byte: u8,
    },
    /// Print registers to stdout, hex and decimal. Debugging aid: what the
    /// guest returned or what a global holds, without a round-trip to a file.
    Print {
        /// Registers to print, `$name` each.
        regs: Vec<String>,
    },
    /// Store a register's value as little-endian `u32`. Links forged structs:
    /// a pointer the guest handed out (`malloc`) goes where the guest will
    /// read it, which no static hex can name.
    Store {
        /// Where to store: `$register` or literal address.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Register whose value is stored, `$name`.
        reg: String,
    },
    /// Add a byte offset to a register. Pointer arithmetic without leaving
    /// the spec: block ends, struct fields past a dynamic base.
    Add {
        /// Register holding the base, `$name`.
        reg: String,
        /// Offset added, checked against overflow.
        by: u32,
        /// Register receiving the sum.
        #[serde(rename = "as")]
        as_reg: String,
    },
    /// Write a host file's bytes at a pointer.
    WriteFile {
        /// Where to write: `$register` or literal address.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Host file whose bytes are copied in.
        file: PathBuf,
    },
    /// Call an exported function; `i32` results land in registers.
    Call {
        /// Export name.
        func: String,
        /// Arguments, `$register` or `i32` literal each.
        #[serde(default)]
        args: Vec<Arg>,
        /// Registers receiving the `i32` results, in order.
        #[serde(default)]
        results: Vec<String>,
    },
    /// Capture an i32 value or f32 bit pattern without dereferencing it.
    CaptureValue {
        /// Selector for the function being observed.
        func: String,
        /// Zero-based operator ordinal in the original body.
        instruction: usize,
        /// Local to read at this boundary.
        local: u32,
        /// Reinterpret f32 bits as i32 for the marker sink.
        #[serde(default)]
        float: bool,
        /// Required hit count.
        count: usize,
        /// Prefix for numbered four-byte outputs.
        out: String,
    },
    /// Capture a guest memory span at one instruction boundary. The marker
    /// only reads an i32 local; its host sink copies memory without writing it.
    /// Configured before execution; all expected hits must occur exactly.
    CaptureMemory {
        /// Selector for the function being observed.
        func: String,
        /// Zero-based operator ordinal in the original body.
        instruction: usize,
        /// i32 local holding the base pointer at this boundary.
        local: u32,
        /// Offset from that pointer.
        #[serde(default)]
        at: u32,
        /// Bytes copied per hit.
        len: u32,
        /// Required number of hits; extra and missing hits fail the run.
        count: usize,
        /// Prefix for numbered binary outputs.
        out: String,
    },
    /// Call a selected function through an added export, including leaves
    /// absent from the function table. Only the export section is changed.
    CallFunction {
        /// Selector declared in `Spec::functions`.
        func: String,
        /// Typed arguments; register references hold i32 values.
        #[serde(default)]
        args: Vec<Arg>,
        /// Registers receiving i32 results.
        #[serde(default)]
        results: Vec<String>,
    },
    /// Refuse a run when a guest status or measured scalar differs.
    AssertReg {
        /// Register reference, including its `$` prefix.
        reg: String,
        /// Expected i32 bit pattern.
        equals: i32,
    },
    /// Call a resolved function through the table. Refuses when the function
    /// sits in any number of slots other than exactly one, unless `slot`
    /// names which one — calling the wrong entry is silent corruption.
    CallTable {
        /// Selector name, as declared in `Spec::functions`.
        func: String,
        /// Arguments, `$register` or `i32` literal each.
        #[serde(default)]
        args: Vec<Arg>,
        /// Registers receiving the `i32` results, in order.
        #[serde(default)]
        results: Vec<String>,
        /// Which slot to call when the function sits in several.
        slot: Option<u32>,
    },
    /// Read guest memory to an output file.
    Read {
        /// Where to read from: `$register` or literal address.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Bytes to read: literal, or `$register` with a guest-reported length.
        len: Len,
        /// Output path, relative to the output directory.
        out: PathBuf,
    },
    /// Load one little-endian `u32` into a register, for chained calls.
    ReadU32 {
        /// Where to read from: `$register` or literal address.
        ptr: Arg,
        /// Byte offset added to the pointer. Struct fields without extra registers.
        #[serde(default)]
        at: u32,
        /// Register receiving the word.
        #[serde(rename = "as")]
        as_reg: String,
    },
    /// Dump raw data-segment bytes to an output file. Purely static: needs no
    /// instance, so table extraction works without executing anything.
    DumpData {
        /// Data segment index, in declaration order.
        segment: usize,
        /// First byte within the segment.
        offset: usize,
        /// Bytes to dump.
        len: usize,
        /// Output path, relative to the output directory.
        out: PathBuf,
    },
    /// Assert a written file's SHA-256. A self-checking spec fails here rather
    /// than publishing a drifted vector.
    AssertSha256 {
        /// Output file to check, relative to the output directory.
        file: PathBuf,
        /// Expected SHA-256, hex.
        sha256: String,
    },
    /// Print the engine log ring to stdout. Whatever the module logged so far:
    /// error codes, branch names, the transcript a bare return code hides.
    Log,
    /// Print recorded host calls to one import, decoding chosen arguments as
    /// C strings. How to read `loggingCallback_js_sync`: the message pointer
    /// is only meaningful while guest memory is alive, i.e. right here.
    Calls {
        /// Import symbol, e.g. `env::loggingCallback_js_sync`.
        import: String,
        /// Argument positions to decode with `read_cstr`, in order.
        #[serde(default)]
        strings: Vec<usize>,
        /// Clear the trace first, so the list (capped at `MAX_TRACE`) holds
        /// this stretch rather than startup's millions of calls.
        #[serde(default)]
        clear_before: bool,
    },
    /// Print instrumented markers reached so far, in order. Pairs with
    /// `oracle instrument`: run the marked copy through a spec pinning it,
    /// and this says which call sites actually ran. Correlate ids with the
    /// marker map `instrument` printed when it built the copy.
    Markers,
    /// Arm recording for one import's markers. Without this the instrumented
    /// copy runs silently: a marker that fired is indistinguishable from one
    /// that was never watched.
    Watch {
        /// Import symbol the markers call, e.g. `env::on_call_event_js_sync`.
        import: String,
    },
}

/// A derivation spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    /// The bytes to run against.
    pub module: ModulePin,
    /// Named function selectors, resolved before the first step.
    #[serde(default)]
    pub functions: BTreeMap<String, FuncSelector>,
    /// Program to execute, in order.
    pub steps: Vec<Step>,
}

/// What `derive` ran and produced. Written as `manifest.json` beside the
/// outputs, with no timestamps: the same spec over the same bytes yields the
/// same manifest byte for byte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// SHA-256 of the spec file, so the manifest names its own program.
    pub spec_sha256: String,
    /// What was actually run.
    pub module: ModuleReport,
    /// How each selector resolved.
    pub resolutions: BTreeMap<String, Resolved>,
    /// Every output file, in write order.
    pub outputs: Vec<OutputReport>,
    /// The oracle that produced this.
    pub oracle: String,
}

/// Module identity, as verified before instantiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleReport {
    /// Module id.
    pub id: String,
    /// Verified SHA-256, hex.
    pub sha256: String,
    /// Verified size in bytes.
    pub size: u64,
}

/// One written file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputReport {
    /// Path relative to the output directory.
    pub file: String,
    /// Bytes written.
    pub bytes: u64,
    /// SHA-256 of the bytes, hex.
    pub sha256: String,
}

/// SHA-256 of bytes, hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A minimal module for tests: one empty function plus one data segment
/// holding a known marker. Built with the same encoder `patch.rs` uses.
#[cfg(test)]
pub(crate) fn probe_module_bytes() -> Vec<u8> {
    use wasm_encoder::{CodeSection, DataSection, FunctionSection, Module, TypeSection};

    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function([], []);
    module.section(&types);
    let mut funcs = FunctionSection::new();
    funcs.function(0);
    module.section(&funcs);
    let mut code = CodeSection::new();
    // Long enough to fingerprint: bodies below FINGERPRINT_FLOOR collide and
    // carry refuses them, so an empty function would make every migration
    // test meaningless.
    let mut function = wasm_encoder::Function::new([]);
    for _ in 0..32 {
        function.instruction(&wasm_encoder::Instruction::Nop);
    }
    function.instruction(&wasm_encoder::Instruction::End);
    code.function(&function);
    module.section(&code);
    let mut data = DataSection::new();
    data.active(
        0,
        &wasm_encoder::ConstExpr::i32_const(0),
        b"probe:opus_codec_encode:tail".to_vec(),
    );
    module.section(&data);
    module.finish()
}

/// Decode even-length hex. Odd length, non-hex digits and `0x` prefixes are
/// refused rather than trimmed: a vector input is exactly its bytes.
fn decode_hex(raw: &str) -> Result<Vec<u8>> {
    hex::decode(raw).with_context(|| format!("decoding {} hex chars", raw.len()))
}

/// Resolve every selector in `spec` against `bytes`, refusing on anything
/// that is not a unique, independently-settled answer.
pub fn resolve_all(bytes: &[u8], spec: &Spec) -> Result<BTreeMap<String, Resolved>> {
    let count = crate::abi::function_count(bytes)?;
    let mut out = BTreeMap::new();
    for (name, selector) in &spec.functions {
        out.insert(name.clone(), resolve_one(bytes, name, selector, count)?);
    }
    for step in &spec.steps {
        let name = match step {
            Step::CallTable { func, .. }
            | Step::CallFunction { func, .. }
            | Step::CaptureMemory { func, .. }
            | Step::CaptureValue { func, .. } => func,
            _ => continue,
        };
        anyhow::ensure!(
            out.contains_key(name),
            "unknown or unresolved function `{name}`"
        );
    }
    Ok(out)
}

/// Resolve one selector.
fn resolve_one(
    bytes: &[u8],
    name: &str,
    selector: &FuncSelector,
    count: usize,
) -> Result<Resolved> {
    let hint = selector.index_hint;
    if hint as usize >= count {
        anyhow::bail!(
            "selector `{name}`: index_hint {hint} is outside this module ({count} functions) — \
             re-derive it, do not guess"
        );
    }

    if let Some(needle) = selector.must_hold_string.as_deref() {
        let refs = crate::abi::find_string_refs(bytes, needle)?;
        let holders: Vec<u32> = refs
            .iter()
            .flat_map(|entry| entry.referenced_by.iter().copied())
            .collect();
        if holders.is_empty() {
            anyhow::bail!("selector `{name}`: no function references {needle:?} — re-derive it");
        }
        if !holders.contains(&hint) {
            anyhow::bail!(
                "selector `{name}`: index_hint {hint} does not reference {needle:?} \
                 (holders: {holders:?}) — the capture moved, re-derive it"
            );
        }
    }

    let fingerprint = body_fingerprint(bytes, hint)?;
    if let Some(expected) = selector.expect_fingerprint {
        match fingerprint {
            Some(actual) if actual == expected => {}
            _ => anyhow::bail!(
                "selector `{name}`: body changed (expected fingerprint {expected}, \
                 found {fingerprint:?}) — re-derive it"
            ),
        }
    }

    let slots = crate::abi::table_slots_of(bytes, hint)?;
    Ok(Resolved {
        index: hint,
        slots,
        fingerprint,
    })
}

/// Body fingerprint of one function, or `None` when it is too short to hash
/// (imports and stubs collide below the floor, so a match there would be
/// worse than no answer).
pub(crate) fn body_fingerprint(bytes: &[u8], func: u32) -> Result<Option<u64>> {
    use unwasm_core::analysis::{FINGERPRINT_FLOOR, fingerprint};
    use unwasm_core::module::Module;

    let module =
        Module::parse(bytes).map_err(|error| anyhow::anyhow!("decoding module: {error}"))?;
    let base = module.func_imports.len() as u32;
    let ordinal = hint_ordinal(func, base, module.funcs.len())?;
    let Some(target) = module.funcs.get(ordinal) else {
        return Ok(None);
    };
    if target.body.len() < FINGERPRINT_FLOOR {
        return Ok(None);
    }
    Ok(Some(fingerprint(&module, target)))
}

/// Defined-function ordinal for a function-space index: imports occupy the low
/// indices, so counting from zero quietly lands on the wrong body.
fn hint_ordinal(hint: u32, import_base: u32, defined: usize) -> Result<usize> {
    let ordinal = hint
        .checked_sub(import_base)
        .context("selector points at an import, which has no body to fingerprint")?;
    if ordinal as usize >= defined {
        anyhow::bail!("selector points past the last defined function");
    }
    Ok(ordinal as usize)
}

/// Runs `spec` from `spec_path` against the module file at `module_path`,
/// writing outputs and `manifest.json` into `out_dir`.
pub fn run_spec(spec_path: &Path, module_path: &Path, out_dir: &Path) -> Result<Manifest> {
    let spec_bytes =
        std::fs::read(spec_path).with_context(|| format!("reading {}", spec_path.display()))?;
    let spec_sha256 = sha256_hex(&spec_bytes);
    let spec: Spec = serde_json::from_slice(&spec_bytes)
        .with_context(|| format!("parsing {}", spec_path.display()))?;

    let module_bytes =
        std::fs::read(module_path).with_context(|| format!("reading {}", module_path.display()))?;
    verify_pin(&spec.module, module_path, &module_bytes)?;

    let resolutions = resolve_all(&module_bytes, &spec)?;

    let mut executor = Executor::new(out_dir);
    executor.execute(&module_bytes, &spec, &resolutions)?;

    let manifest = Manifest {
        spec_sha256,
        module: ModuleReport {
            id: spec.module.id.clone(),
            sha256: spec.module.sha256.clone(),
            size: spec.module.size,
        },
        resolutions,
        outputs: executor.outputs,
        oracle: format!("oracle-core {}", env!("CARGO_PKG_VERSION")),
    };
    let manifest_bytes = serde_json::to_string_pretty(&manifest).context("encoding manifest")?;
    std::fs::write(out_dir.join("manifest.json"), &manifest_bytes)
        .with_context(|| format!("writing {}", out_dir.join("manifest.json").display()))?;
    Ok(manifest)
}

/// Refuse any bytes that are not exactly the pinned capture.
pub(crate) fn verify_pin(pin: &ModulePin, path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() as u64 != pin.size {
        anyhow::bail!(
            "module size mismatch for `{}`: spec pins {} bytes, {} has {}",
            pin.id,
            pin.size,
            path.display(),
            bytes.len()
        );
    }
    let actual = sha256_hex(bytes);
    if actual != pin.sha256 {
        anyhow::bail!(
            "module hash mismatch for `{}`: spec pins {}, {} has {actual}",
            pin.id,
            pin.sha256,
            path.display()
        );
    }
    Ok(())
}

/// Executes a spec's steps.
struct Executor<'a> {
    out_dir: &'a Path,
    runtime: Option<crate::Runtime>,
    regs: HashMap<String, u32>,
    outputs: Vec<OutputReport>,
    snapshot_config: Option<(String, BTreeMap<i32, crate::snapshot::Span>)>,
}

impl<'a> Executor<'a> {
    /// Creates an executor writing into `out_dir`, creating it first.
    fn new(out_dir: &'a Path) -> Self {
        Self {
            out_dir,
            runtime: None,
            regs: HashMap::new(),
            outputs: Vec::new(),
            snapshot_config: None,
        }
    }

    /// Runs every step in order. Resolution happened before this, so every
    /// named function is known good by the time a call runs.
    fn execute(
        &mut self,
        module_bytes: &[u8],
        spec: &Spec,
        resolutions: &BTreeMap<String, Resolved>,
    ) -> Result<()> {
        std::fs::create_dir_all(self.out_dir)
            .with_context(|| format!("creating {}", self.out_dir.display()))?;
        let mut plan = crate::patch::Plan::default();
        let mut spans = Vec::new();
        for step in &spec.steps {
            let (func, instruction, local, at, len, count, out, scalar, float) = match step {
                Step::CaptureMemory {
                    func,
                    instruction,
                    local,
                    at,
                    len,
                    count,
                    out,
                } => (
                    func,
                    *instruction,
                    *local,
                    *at,
                    *len,
                    *count,
                    out,
                    false,
                    false,
                ),
                Step::CaptureValue {
                    func,
                    instruction,
                    local,
                    float,
                    count,
                    out,
                } => (func, *instruction, *local, 0, 4, *count, out, true, *float),
                _ => continue,
            };
            let resolved = resolutions
                .get(func)
                .with_context(|| format!("unknown snapshot function {func}"))?;
            anyhow::ensure!(
                !out.is_empty() && out.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                "invalid snapshot prefix"
            );
            anyhow::ensure!(
                !spans.iter().any(|s: &crate::snapshot::Span| s.out == *out),
                "duplicate snapshot prefix {out}"
            );
            plan.value_at
                .push((resolved.index, instruction, local, float));
            spans.push(crate::snapshot::Span {
                at,
                len,
                count,
                out: out.clone(),
                scalar,
            });
        }
        let patched;
        let module_bytes = if spans.is_empty() {
            module_bytes
        } else {
            let (bytes, map) = crate::patch::instrument(module_bytes, &plan)?;
            self.snapshot_config = Some((
                map.via_symbol.clone(),
                map.markers.iter().map(|m| m.id).zip(spans).collect(),
            ));
            patched = bytes;
            &patched
        };
        for (position, step) in spec.steps.iter().enumerate() {
            step.execute(position, module_bytes, resolutions, self)
                .with_context(|| format!("step {} ({step:?})", position + 1))?;
        }
        if let Some(runtime) = &self.runtime
            && let Some(recorder) = runtime.shared().snapshots.get()
        {
            for (name, bytes) in recorder.finish()? {
                self.record_output(Path::new(&name), &bytes)?;
            }
        }
        Ok(())
    }

    /// The live instance, or an error naming the step that forgot to create it.
    fn live(&mut self) -> Result<&mut crate::Runtime> {
        self.runtime
            .as_mut()
            .context("no instance: the spec needs an `instantiate` step before this one")
    }

    /// Resolve an argument to an `i32` for a guest call.
    fn arg_value(&self, arg: &Arg) -> Result<i32> {
        match arg {
            Arg::Reg(name) => {
                let bare = name
                    .strip_prefix('$')
                    .with_context(|| format!("register `{name}` must start with `$`"))?;
                self.regs
                    .get(bare)
                    .copied()
                    .map(|value| value as i32)
                    .with_context(|| format!("register `${bare}` is not set"))
            }
            Arg::Lit(value) => Ok(*value),
            Arg::Float32 { .. } => anyhow::bail!("float argument used as an integer or pointer"),
        }
    }

    fn call_value(&self, arg: &Arg) -> Result<Val> {
        match arg {
            Arg::Float32 { f32: value } => Ok(Val::F32(value.to_bits())),
            _ => self.arg_value(arg).map(Val::I32),
        }
    }

    /// Resolve a pointer argument to a `u32` address.
    fn ptr_value(&self, arg: &Arg) -> Result<u32> {
        self.arg_value(arg).map(|value| value as u32)
    }

    /// Resolve a pointer argument plus a byte offset, checked against overflow.
    fn ptr_at(&self, arg: &Arg, at: u32) -> Result<u32> {
        let base = self.ptr_value(arg)?;
        base.checked_add(at)
            .context("pointer plus offset overflows u32")
    }

    /// Store call results (all `i32`) into named registers.
    fn store_results(&mut self, names: &[String], values: Vec<Val>) -> Result<()> {
        if values.len() != names.len() {
            anyhow::bail!(
                "guest returned {} value(s), spec names {} register(s)",
                values.len(),
                names.len()
            );
        }
        for (name, value) in names.iter().zip(values) {
            let Val::I32(word) = value else {
                anyhow::bail!("result for `${name}` is not an i32 ({value:?})");
            };
            self.regs.insert(name.clone(), word as u32);
        }
        Ok(())
    }

    /// Record a written output file for the manifest.
    fn record_output(&mut self, file: &Path, bytes: &[u8]) -> Result<()> {
        let full = self.out_dir.join(file);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&full, bytes).with_context(|| format!("writing {}", full.display()))?;
        self.outputs.push(OutputReport {
            file: file.to_string_lossy().into_owned(),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
        Ok(())
    }
}

/// Expose selected leaves without rewriting code, tables, or function indices.
fn export_selectors(bytes: &[u8], resolutions: &BTreeMap<String, Resolved>) -> Result<Vec<u8>> {
    use wasm_encoder::{ExportKind, ExportSection, Module, RawSection};
    use wasmparser::{ExternalKind, Parser, Payload};
    let mut exports = ExportSection::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(section) = payload? {
            for entry in section {
                let entry = entry?;
                anyhow::ensure!(
                    !entry.name.starts_with("__derive_"),
                    "reserved derivation export: {}",
                    entry.name
                );
                let kind = match entry.kind {
                    ExternalKind::Func => ExportKind::Func,
                    ExternalKind::Table => ExportKind::Table,
                    ExternalKind::Memory => ExportKind::Memory,
                    ExternalKind::Global => ExportKind::Global,
                    ExternalKind::Tag => ExportKind::Tag,
                    other => anyhow::bail!("unsupported export kind: {other:?}"),
                };
                exports.export(entry.name, kind, entry.index);
            }
        }
    }
    for (name, resolved) in resolutions {
        exports.export(
            &format!("__derive_{name}"),
            ExportKind::Func,
            resolved.index,
        );
    }
    let mut module = Module::new();
    let mut inserted = false;
    for payload in Parser::new(0).parse_all(bytes) {
        if let Some((id, range)) = payload?.as_section() {
            if id >= 7 && !inserted {
                module.section(&exports);
                inserted = true;
            }
            if id != 7 {
                module.section(&RawSection {
                    id,
                    data: &bytes[range],
                });
            }
        }
    }
    if !inserted {
        module.section(&exports);
    }
    Ok(module.finish())
}

/// Enrich a failed guest call with the markers recorded so far.
///
/// A trap discards the run, and with it the trace of how far the guest got.
/// Markers survive in shared state, so attaching them turns "it trapped
/// somewhere in there" into "it reached these call sites, in this order".
fn describe_guest_error(
    executor: &mut Executor<'_>,
    what: &str,
    error: anyhow::Error,
) -> anyhow::Error {
    let markers: Vec<(i32, i64)> = executor
        .live()
        .map(|live| live.shared().markers())
        .unwrap_or_default();
    if markers.is_empty() {
        return error.context(what.to_owned());
    }
    let trail: Vec<String> = markers
        .iter()
        .map(|(id, value)| format!("{id}:{value}"))
        .collect();
    error.context(format!("{what} (markers so far: {})", trail.join(" ")))
}

impl Step {
    /// Execute one step.
    fn execute(
        &self,
        _position: usize,
        module_bytes: &[u8],
        resolutions: &BTreeMap<String, Resolved>,
        executor: &mut Executor<'_>,
    ) -> Result<()> {
        match self {
            Step::Instantiate => {
                // Single-threaded: thread scheduling is the one thing that
                // makes two runs of the same bytes differ.
                let exported = export_selectors(module_bytes, resolutions)?;
                let runtime = crate::Runtime::instantiate(&exported)?;
                if let Some((symbol, spans)) = executor.snapshot_config.take() {
                    runtime
                        .shared()
                        .snapshots
                        .set(crate::snapshot::Recorder::new(symbol, spans))
                        .map_err(|_| anyhow::anyhow!("snapshot recorder already configured"))?;
                }
                executor.runtime = Some(runtime);
                Ok(())
            }
            Step::RunCtors => {
                executor.live()?.run_ctors()?;
                Ok(())
            }
            Step::LogRing { len } => {
                executor.live()?.attach_log_ring(*len)?;
                Ok(())
            }
            Step::Malloc { as_reg, len } => {
                let ptr = executor.live()?.malloc(*len)?;
                executor.regs.insert(as_reg.clone(), ptr);
                Ok(())
            }
            Step::Write { ptr, at, hex } => {
                let address = executor.ptr_at(ptr, *at)?;
                let bytes = decode_hex(hex)?;
                executor.live()?.write_bytes_at(address, &bytes)?;
                Ok(())
            }
            Step::WriteFile { ptr, at, file } => {
                let address = executor.ptr_at(ptr, *at)?;
                let bytes = std::fs::read(file)
                    .with_context(|| format!("reading input {}", file.display()))?;
                executor.live()?.write_bytes_at(address, &bytes)?;
                Ok(())
            }
            Step::Fill { ptr, at, len, byte } => {
                let address = executor.ptr_at(ptr, *at)?;
                executor
                    .live()?
                    .write_bytes_at(address, &vec![*byte; *len as usize])?;
                Ok(())
            }
            Step::Print { regs } => {
                for name in regs {
                    let bare = name
                        .strip_prefix('$')
                        .with_context(|| format!("register `{name}` must start with `$`"))?;
                    let value = executor
                        .regs
                        .get(bare)
                        .copied()
                        .with_context(|| format!("register `${bare}` is not set"))?;
                    println!("  ${bare} = {value:#x} ({value})");
                }
                Ok(())
            }
            Step::Store { ptr, at, reg } => {
                let address = executor.ptr_at(ptr, *at)?;
                let bare = reg
                    .strip_prefix('$')
                    .with_context(|| format!("register `{reg}` must start with `$`"))?;
                let value = executor
                    .regs
                    .get(bare)
                    .copied()
                    .with_context(|| format!("register `${bare}` is not set"))?;
                executor
                    .live()?
                    .write_bytes_at(address, &value.to_le_bytes())?;
                Ok(())
            }
            Step::Add { reg, by, as_reg } => {
                let bare = reg
                    .strip_prefix('$')
                    .with_context(|| format!("register `{reg}` must start with `$`"))?;
                let base = executor
                    .regs
                    .get(bare)
                    .copied()
                    .with_context(|| format!("register `${bare}` is not set"))?;
                let sum = base
                    .checked_add(*by)
                    .context("register plus offset overflows u32")?;
                executor.regs.insert(as_reg.clone(), sum);
                Ok(())
            }
            Step::Call {
                func,
                args,
                results,
            } => {
                let values: Vec<Val> = args
                    .iter()
                    .map(|arg| executor.call_value(arg))
                    .collect::<Result<_>>()?;
                let returned = executor.live()?.call(func, &values).map_err(|error| {
                    describe_guest_error(executor, &format!("calling `{func}`"), error)
                })?;
                executor.store_results(results, returned)?;
                Ok(())
            }
            Step::CaptureMemory { .. } | Step::CaptureValue { .. } => Ok(()),
            Step::AssertReg { reg, equals } => {
                let actual = executor.arg_value(&Arg::Reg(reg.clone()))?;
                anyhow::ensure!(actual == *equals, "{reg}: expected {equals}, got {actual}");
                Ok(())
            }
            Step::CallFunction {
                func,
                args,
                results,
            } => {
                anyhow::ensure!(resolutions.contains_key(func), "unknown function `{func}`");
                let values = args
                    .iter()
                    .map(|arg| executor.call_value(arg))
                    .collect::<Result<Vec<_>>>()?;
                let returned = executor
                    .live()?
                    .call(&format!("__derive_{func}"), &values)
                    .map_err(|error| {
                        describe_guest_error(executor, &format!("calling selected `{func}`"), error)
                    })?;
                executor.store_results(results, returned)
            }
            Step::CallTable {
                func,
                args,
                results,
                slot,
            } => {
                let resolved = resolutions
                    .get(func)
                    .with_context(|| format!("unknown function `{func}`"))?;
                let slot = match slot {
                    Some(slot) => {
                        if !resolved.slots.contains(slot) {
                            anyhow::bail!(
                                "function `{func}` (index {}) is not in slot {slot} \
                                 (slots: {:?})",
                                resolved.index,
                                resolved.slots
                            );
                        }
                        *slot
                    }
                    None => match resolved.slots.as_slice() {
                        [only] => *only,
                        other => anyhow::bail!(
                            "function `{func}` sits in {} slots ({other:?}): \
                             name one with `slot`, guessing is not an option",
                            other.len()
                        ),
                    },
                };
                let values: Vec<Val> = args
                    .iter()
                    .map(|arg| executor.call_value(arg))
                    .collect::<Result<_>>()?;
                let returned = executor
                    .live()?
                    .call_table(slot, &values)
                    .map_err(|error| {
                        describe_guest_error(executor, &format!("calling table slot {slot}"), error)
                    })?;
                executor.store_results(results, returned)?;
                Ok(())
            }
            Step::Read { ptr, at, len, out } => {
                let address = executor.ptr_at(ptr, *at)?;
                let len = match len {
                    Len::Lit(len) => *len,
                    Len::Reg(name) => {
                        let bare = name.strip_prefix('$').with_context(|| {
                            format!("length register `{name}` must start with `$`")
                        })?;
                        executor
                            .regs
                            .get(bare)
                            .copied()
                            .with_context(|| format!("length register `${bare}` is not set"))?
                    }
                };
                let bytes = executor.live()?.read(address, len)?;
                executor.record_output(out, &bytes)?;
                Ok(())
            }
            Step::ReadU32 { ptr, at, as_reg } => {
                let address = executor.ptr_at(ptr, *at)?;
                let bytes = executor.live()?.read(address, 4)?;
                let word =
                    u32::from_le_bytes(bytes.as_slice().try_into().ok().context(
                        "short read loading a u32 (guest memory changed under the read)",
                    )?);
                executor.regs.insert(as_reg.clone(), word);
                Ok(())
            }
            Step::DumpData {
                segment,
                offset,
                len,
                out,
            } => {
                let bytes = dump_data(module_bytes, *segment, *offset, *len)?;
                executor.record_output(out, &bytes)?;
                Ok(())
            }
            Step::AssertSha256 { file, sha256 } => {
                let bytes = std::fs::read(executor.out_dir.join(file))
                    .with_context(|| format!("reading output {}", file.display()))?;
                let actual = sha256_hex(&bytes);
                if actual != *sha256 {
                    anyhow::bail!(
                        "self-check failed for {}: spec expects {sha256}, got {actual}",
                        file.display()
                    );
                }
                Ok(())
            }
            Step::Log => {
                let live = executor.live()?;
                for line in live.logs() {
                    println!("  hlog: {line}");
                }
                for line in live.engine_log() {
                    println!("  log: {line}");
                }
                Ok(())
            }
            Step::Calls {
                import,
                strings,
                clear_before,
            } => {
                let live = executor.live()?;
                if *clear_before {
                    live.clear_calls();
                }
                for call in live.all_calls_to(import) {
                    println!("  {}::{} {:?}", call.module, call.name, call.args);
                    for position in strings {
                        if let Some(arg) = call.args.get(*position) {
                            let ptr = u32::try_from(*arg).unwrap_or(0);
                            match live.read_cstr(ptr) {
                                Ok(text) => println!("    arg{position}: {text:?}"),
                                Err(error) => println!("    arg{position}: unreadable ({error:#})"),
                            }
                        }
                    }
                }
                Ok(())
            }
            Step::Markers => {
                let ids: Vec<i32> = executor
                    .live()?
                    .shared()
                    .markers()
                    .iter()
                    .map(|(id, _)| *id)
                    .collect();
                println!("  markers: {ids:?}");
                Ok(())
            }
            Step::Watch { import } => {
                executor.live()?.shared().watch_markers(import);
                Ok(())
            }
        }
    }
}

/// Raw bytes of one data segment slice, by segment index in declaration order.
fn dump_data(bytes: &[u8], segment: usize, offset: usize, len: usize) -> Result<Vec<u8>> {
    use wasmparser::{Parser, Payload};

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::DataSection(reader) = payload.context("parsing sections")? else {
            continue;
        };
        for (index, data) in reader.into_iter().enumerate() {
            if index != segment {
                continue;
            }
            let data = data.context("reading data segment")?;
            let end = offset.checked_add(len).context("slice end overflows")?;
            if end > data.data.len() {
                anyhow::bail!(
                    "segment {segment} holds {} bytes, slice {offset}..{end} is out of range",
                    data.data.len()
                );
            }
            return Ok(data.data[offset..end].to_vec());
        }
    }
    anyhow::bail!("no data segment {segment} in this module")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_leaf_preserves_float_memory_effects_and_traps() {
        use wasm_encoder::{
            CodeSection, Function, FunctionSection, Instruction, MemorySection, MemoryType, Module,
            TypeSection, ValType,
        };
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([ValType::I32, ValType::F32], []);
        types.ty().function([ValType::I32, ValType::I32], []);
        module.section(&types);
        let mut imports = wasm_encoder::ImportSection::new();
        imports.import(
            "env",
            "on_call_event_js_sync",
            wasm_encoder::EntityType::Function(1),
        );
        module.section(&imports);
        let mut functions = FunctionSection::new();
        functions.function(0);
        module.section(&functions);
        let mut memory = MemorySection::new();
        memory.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        module.section(&memory);
        let mut exports = wasm_encoder::ExportSection::new();
        exports.export("memory", wasm_encoder::ExportKind::Memory, 0);
        module.section(&exports);
        let mut function = Function::new([]);
        function.instruction(&Instruction::LocalGet(0));
        function.instruction(&Instruction::LocalGet(1));
        function.instruction(&Instruction::F32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));
        function.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&function);
        module.section(&code);
        let bytes = module.finish();
        let resolved = BTreeMap::from([(
            "leaf".into(),
            Resolved {
                index: 1,
                slots: vec![],
                fingerprint: None,
            },
        )]);
        let exported = export_selectors(&bytes, &resolved).expect("exports");
        // Every non-export section is preserved byte-for-byte, including code.
        let sections = |bytes: &[u8]| {
            wasmparser::Parser::new(0)
                .parse_all(bytes)
                .filter_map(|p| p.expect("valid").as_section())
                .filter(|(id, _)| *id != 7)
                .map(|(id, range)| (id, bytes[range].to_vec()))
                .collect::<Vec<_>>()
        };
        assert_eq!(sections(&bytes), sections(&exported));
        let mut executor = Executor::new(Path::new("."));
        Step::Instantiate
            .execute(0, &bytes, &resolved, &mut executor)
            .expect("instance");
        let call = Step::CallFunction {
            func: "leaf".into(),
            args: vec![Arg::Lit(16), Arg::Float32 { f32: -1.25 }],
            results: vec![],
        };
        call.execute(1, &bytes, &resolved, &mut executor)
            .expect("call");
        assert_eq!(
            executor.live().expect("live").read(16, 4).expect("read"),
            (-1.25f32).to_le_bytes()
        );
        let call = Step::CallFunction {
            func: "leaf".into(),
            args: vec![Arg::Lit(65535), Arg::Float32 { f32: 1.0 }],
            results: vec![],
        };
        assert!(
            call.execute(2, &bytes, &resolved, &mut executor).is_err(),
            "OOB must remain a trap"
        );
        executor.regs.insert("rc".into(), 7);
        assert!(
            Step::AssertReg {
                reg: "$rc".into(),
                equals: 0
            }
            .execute(3, &bytes, &resolved, &mut executor)
            .is_err()
        );
        assert!(executor.ptr_value(&Arg::Float32 { f32: 16.0 }).is_err());

        let plan = crate::patch::Plan {
            value_at: vec![(1, 0, 0, false), (1, 3, 0, false), (1, 0, 1, true)],
            ..Default::default()
        };
        let (observed_bytes, markers) =
            crate::patch::instrument(&exported, &plan).expect("instrument");
        let make_recorder = |count| {
            crate::snapshot::Recorder::new(
                markers.via_symbol.clone(),
                markers
                    .markers
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        (
                            m.id,
                            crate::snapshot::Span {
                                at: 0,
                                len: 4,
                                count,
                                out: format!("span{i}"),
                                scalar: i == 2,
                            },
                        )
                    })
                    .collect(),
            )
        };
        let mut observed = crate::Runtime::instantiate(&observed_bytes).expect("observed instance");
        observed
            .shared()
            .snapshots
            .set(make_recorder(1))
            .expect("recorder");
        observed
            .call(
                "__derive_leaf",
                &[Val::I32(16), Val::F32((-1.25f32).to_bits())],
            )
            .expect("observed call");
        let captures = observed
            .shared()
            .snapshots
            .get()
            .expect("recorder")
            .finish()
            .expect("exact counts");
        assert_eq!(captures[0].1, [0; 4]);
        assert_eq!(captures[1].1, (-1.25f32).to_le_bytes());
        assert_eq!(captures[2].1, (-1.25f32).to_le_bytes());
        assert_eq!(
            observed.read(16, 4).expect("read"),
            executor
                .live()
                .expect("live")
                .read(16, 4)
                .expect("plain read")
        );
        let mut missing = crate::Runtime::instantiate(&observed_bytes).expect("missing instance");
        missing
            .shared()
            .snapshots
            .set(make_recorder(2))
            .expect("recorder");
        missing
            .call("__derive_leaf", &[Val::I32(16), Val::F32(1.0f32.to_bits())])
            .expect("one call");
        assert!(
            missing
                .shared()
                .snapshots
                .get()
                .expect("recorder")
                .finish()
                .is_err(),
            "missing hits cannot pass"
        );
    }

    /// A minimal module: one empty function plus one data segment holding a
    /// known marker, built with the same encoder `patch.rs` uses.
    fn probe_module() -> Vec<u8> {
        probe_module_bytes()
    }

    /// Resolution settles a hint by an independent fact: a wrong hint against
    /// the same string fails rather than answering about the wrong function.
    #[test]
    fn a_wrong_hint_against_the_same_string_is_refused() {
        let bytes = probe_module();
        let count = crate::abi::function_count(&bytes).expect("count");
        assert_eq!(count, 1);

        let good = FuncSelector {
            index_hint: 0,
            must_hold_string: None,
            expect_fingerprint: None,
        };
        let resolved = resolve_one(&bytes, "probe", &good, count).expect("resolve");
        assert_eq!(resolved.index, 0);
        assert!(resolved.slots.is_empty());

        let bad = FuncSelector {
            index_hint: 7,
            must_hold_string: None,
            expect_fingerprint: None,
        };
        let error = resolve_one(&bytes, "probe", &bad, count).unwrap_err();
        assert!(
            error.to_string().contains("outside this module"),
            "{error:#}"
        );
    }

    /// `dump_data` slices exactly the declared range and refuses the rest.
    #[test]
    fn dump_data_slices_exactly_and_refuses_overreach() {
        let bytes = probe_module();
        let marker = b"probe:opus_codec_encode:tail";
        let whole = dump_data(&bytes, 0, 0, marker.len()).expect("slice");
        assert_eq!(whole, marker);
        assert!(dump_data(&bytes, 0, 0, marker.len() + 1).is_err());
        assert!(dump_data(&bytes, 3, 0, 1).is_err());
    }

    /// Hex inputs are exact bytes: odd length and non-hex are errors, and
    /// register arguments must wear their `$`.
    #[test]
    fn malformed_step_inputs_are_refused() {
        assert_eq!(decode_hex("00ff").expect("hex"), vec![0, 255]);
        assert!(decode_hex("0").is_err());
        assert!(decode_hex("zz").is_err());

        let executor = Executor::new(Path::new("/nonexistent-derive-test"));
        assert!(executor.arg_value(&Arg::Lit(3)).expect("lit") == 3);
        assert!(executor.arg_value(&Arg::Reg("x".to_owned())).is_err());
        assert!(executor.arg_value(&Arg::Reg("$x".to_owned())).is_err());
    }

    /// A `must_hold_string` the module never references fails resolution.
    #[test]
    fn an_unreferenced_string_fails_resolution() {
        let bytes = probe_module();
        let count = crate::abi::function_count(&bytes).expect("count");
        let selector = FuncSelector {
            index_hint: 0,
            must_hold_string: Some("no-such-string".to_owned()),
            expect_fingerprint: None,
        };
        let error = resolve_one(&bytes, "probe", &selector, count).unwrap_err();
        assert!(
            error.to_string().contains("no function references"),
            "{error:#}"
        );
    }
}
