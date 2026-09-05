//! `oracle` — inspect and instantiate WhatsApp Web's captured wasm modules.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use oracle_core::inspect::EntryKind;
use oracle_core::{Catalog, Inspection, Runtime, ThreadPolicy, Value};

#[derive(Parser)]
#[command(name = "oracle", about, version)]
struct Cli {
    /// Directory holding the captured .wasm files. Defaults to the
    /// repository's `.cache/wa-wasm`, or $WA_WASM_DIR.
    #[arg(long, global = true)]
    dir: Option<String>,

    #[command(subcommand)]
    command: Command,
}

/// Options shared by the subcommands that execute a module.
#[derive(clap::Args, Clone, Default)]
struct EngineOpts {
    /// Run guest threads for real. Required by modules whose initialisation
    /// waits on a worker — the VoIP engine's media stack is one.
    #[arg(long, global = true)]
    threads: bool,
    /// Attach a log ring buffer and print what the module logged. The VoIP
    /// engine explains its own failures there.
    #[arg(long, global = true)]
    log: bool,
}

/// Where the markers go, and what they call. Grouped so `instrument` takes a
/// spec rather than seven positional flags.
#[derive(clap::Args)]
struct MarkOpts {
    /// Mark the entry of this function. Repeatable.
    #[arg(long)]
    entry: Vec<u32>,
    /// Mark every direct call site in this function. Repeatable.
    #[arg(long = "calls-in")]
    calls_in: Vec<u32>,
    /// Mark this function's entry, reporting one of its locals as the value:
    /// `FUNC:LOCAL`. For a parameter that is the argument it was called with.
    /// Repeatable.
    #[arg(long)]
    value: Vec<String>,
    /// Mark every `return` in this function. Repeatable.
    #[arg(long = "returns-in")]
    returns_in: Vec<u32>,
    /// The import a marker calls, as `module::name` or a bare name.
    ///
    /// Without this only an import known to be recording-only is used, and a
    /// module that declares none is refused rather than instrumented with one
    /// that writes guest memory. The refusal lists the candidates.
    #[arg(long)]
    sink: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// List the captured modules.
    List,
    /// Show a module's exports, imports, sections and toolchain.
    Inspect {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Also print every export and import, not just a summary.
        #[arg(long, short)]
        full: bool,
    },
    /// Bring a module up and report what its startup did.
    Instantiate {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Run guest threads for real. Needed by modules whose initialisation
        /// waits on a worker; see ThreadPolicy.
        #[arg(long)]
        threads: bool,
    },
    /// Run a module's constructors and list the embind API it registers.
    Embind {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Show class methods as well as free functions.
        #[arg(long, short)]
        full: bool,
        #[command(flatten)]
        engine: EngineOpts,
    },
    /// Call an embind function and print what it returns.
    ///
    /// Arguments are parsed by the function's registered parameter types:
    /// integers and bools by value, everything else as a string.
    Call {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Registered function name, e.g. getWebP2PVirtualIpv4.
        function: String,
        /// Arguments, in declaration order.
        args: Vec<String>,
        #[command(flatten)]
        engine: EngineOpts,
    },
    /// Run a WASI module as the command-line tool it is.
    ///
    /// Input files are copied into an in-memory filesystem before the run, and
    /// anything the tool writes can be copied back out. The guest never touches
    /// the real filesystem.
    Run {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Arguments passed to the guest, after `--`.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Host file to place in the guest filesystem, as `guest=host` or a
        /// plain path (used under its own file name). Repeatable.
        #[arg(long = "file", short = 'f')]
        files: Vec<String>,
        /// Guest file to write out afterwards, as `guest=host`. Repeatable.
        #[arg(long = "out", short = 'o')]
        outputs: Vec<String>,
        #[command(flatten)]
        engine: EngineOpts,
    },
    /// Infer what a function's parameters are, from its own bytecode.
    ///
    /// Works on stripped and minified modules: it reads what the code does with
    /// each argument rather than relying on names or glue.
    Abi {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Only functions whose name contains this. Omit for all exports.
        #[arg(long, short)]
        filter: Option<String>,
        /// Follow a function-table slot instead of an export. Use this to see
        /// through a trampoline, whose real arguments belong to its callee.
        #[arg(long)]
        slot: Option<u32>,
        /// Look up one function by its index in the module's function space.
        /// This is the form a wasm backtrace uses, so it is how a trap frame
        /// ("wasm function 151") becomes a name and a signature.
        #[arg(long)]
        index: Option<u32>,
        /// List up to this many instructions per function. Raise it to read a
        /// body rather than just its summary.
        #[arg(long, default_value_t = 24)]
        body: usize,
    },
    /// Find the code that reaches for a given string.
    ///
    /// A log message is the most legible thing a stripped module emits, and it
    /// names the branch that produced it. This goes from the message back to
    /// the functions that mention it, which `abi --index` can then read.
    Xref {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Text to look for. Matched as a substring.
        text: String,
    },
    /// Find the code that reaches an address, directly or as a table base.
    XrefAddr {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Address, decimal or `0x`-prefixed.
        address: String,
        /// How far back a table base may sit, in bytes.
        #[arg(long, default_value_t = 4096)]
        window: u32,
    },
    /// List the functions that call a given function.
    Callers {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Function index, as a backtrace or `xref` reports it.
        index: u32,
    },
    /// Read a table of string pointers as the named enum it usually is.
    ///
    /// An enum's name table is in declaration order, so an entry's index is the
    /// enum value — which turns `call_term_reason: 27` into a name.
    Enum {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Table base address, decimal or `0x`-prefixed.
        base: String,
        /// How many entries to read.
        #[arg(long, default_value_t = 64)]
        count: u32,
    },
    /// List the functions that load a given constant.
    ///
    /// A callback reaches the guest as a table index — an ordinary
    /// `i32.const` that nothing calls and nothing points at. This is how to
    /// trace one back to whatever dispatches it.
    Konst {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// The constant to look for.
        value: i32,
    },
    /// Carry a function index from one capture to the next.
    ///
    /// WhatsApp renumbers every function between rollouts, so an index recorded
    /// against one binary answers about different code in the next — and the
    /// read still succeeds, which is what makes it dangerous. This finds the
    /// same function again by the shape of its body, using the decompiler's
    /// fingerprint: it drops constant values, call targets and global indices,
    /// which are exactly what a rebuild changes.
    ///
    /// An answer is only given when it is unique on both sides. Ambiguous and
    /// changed are reported as such rather than guessed.
    Carry {
        /// The capture the index was read out of.
        old: String,
        /// The capture to find it in.
        new: String,
        /// Function indices from the old capture.
        indices: Vec<u32>,
    },
    /// Print printable runs found in the module's data segments.
    Strings {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Minimum run length to report.
        #[arg(long, default_value_t = 6)]
        min: usize,
    },
    /// Derive vectors from a capture, deterministically.
    ///
    /// Runs a spec — a small program of allocations, writes, calls and reads
    /// against the pinned bytes — and writes the outputs plus a `manifest.json`
    /// recording the module hash, the resolved indices and slots, and every
    /// output's hash. The same spec over the same bytes yields the same bytes.
    Derive {
        /// Path to the spec file (JSON).
        #[arg(long)]
        spec: String,
        /// Directory receiving the outputs and `manifest.json`.
        #[arg(long, short)]
        out: String,
    },
    /// Carry a derivation spec from one capture to the next.
    ///
    /// WhatsApp renumbers every function between rollouts, so a spec pinned
    /// to one capture answers about different code in the next. This carries
    /// each selector forward by body fingerprint, re-settles it against its
    /// string anchor on the new bytes, and writes the migrated spec — plus a
    /// report saying what carried and what needs a human. Anything that is
    /// not a unique answer on both sides is refused, never guessed.
    Migrate {
        /// Path to the spec file written against the old capture.
        #[arg(long)]
        spec: String,
        /// Old capture: module id (as shown by `list`) or path to a `.wasm`.
        #[arg(long)]
        from: String,
        /// New capture: module id or path to a `.wasm`.
        #[arg(long)]
        to: String,
        /// Expected SHA-256 of the new capture, hex. From a trusted lock —
        /// a hash that vouches for itself vouches for nothing.
        #[arg(long = "new-sha")]
        new_sha: String,
        /// Expected size of the new capture, in bytes.
        #[arg(long = "new-size")]
        new_size: u64,
        /// Where to write the migrated spec.
        #[arg(long, short)]
        out: String,
    },
    /// Write a copy with trace markers spliced in, and print the marker map.
    ///
    /// A marker is a call to an import the module already declares, so nothing
    /// is renumbered. Run the copy and read `Runtime::shared().markers()` — or
    /// use `--example free_trap` — to see which of them were reached, in order.
    ///
    /// This answers what `abi` cannot: it lists a function's ten call sites and
    /// says nothing about which one ran.
    Instrument {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Where to write the instrumented copy.
        #[arg(long, short)]
        out: String,
        #[command(flatten)]
        marks: MarkOpts,
    },
    /// Write a copy with instructions replaced, for forcing a gate open.
    ///
    /// Each `--replace` is `FUNC:AT:COUNT:SPEC`, where AT is the operator's
    /// position in the body — the order `abi --index` prints — and SPEC is
    /// `;`-separated `drop`, `nop`, `i32.const N` or `i64.const N`. The
    /// replacement must leave the stack as it found it.
    Patch {
        /// Module id (as shown by `list`) or a path to a .wasm file.
        target: String,
        /// Where to write the patched copy.
        #[arg(long, short)]
        out: String,
        /// FUNC:AT:COUNT:SPEC. Repeatable.
        #[arg(long)]
        replace: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let catalog = match &cli.dir {
        Some(dir) => Catalog::from_dir(dir)?,
        None => Catalog::discover()?,
    };

    match cli.command {
        Command::List => list(&catalog),
        Command::Inspect { target, full } => inspect(&catalog, &target, full),
        Command::Instantiate { target, threads } => instantiate(&catalog, &target, threads),
        Command::Strings { target, min } => strings(&catalog, &target, min),
        Command::Carry { old, new, indices } => carry(&catalog, &old, &new, &indices),
        Command::Instrument { target, out, marks } => instrument(&catalog, &target, &out, &marks),
        Command::Patch {
            target,
            out,
            replace,
        } => patch_module(&catalog, &target, &out, &replace),
        Command::Xref { target, text } => xref(&catalog, &target, &text),
        Command::Callers { target, index } => callers(&catalog, &target, index),
        Command::Konst { target, value } => constant_users(&catalog, &target, value),
        Command::Enum {
            target,
            base,
            count,
        } => enum_table(&catalog, &target, &base, count),
        Command::XrefAddr {
            target,
            address,
            window,
        } => xref_addr(&catalog, &target, &address, window),
        Command::Abi {
            target,
            filter,
            slot,
            index,
            body,
        } => abi(&catalog, &target, filter.as_deref(), slot, index, body),
        Command::Embind {
            target,
            full,
            engine,
        } => embind(&catalog, &target, full, &engine),
        Command::Call {
            target,
            function,
            args,
            engine,
        } => call(&catalog, &target, &function, &args, &engine),
        Command::Run {
            target,
            args,
            files,
            outputs,
            engine,
        } => run_wasi(&catalog, &target, &args, &files, &outputs, &engine),
        Command::Derive { spec, out } => derive(&catalog, &spec, &out),
        Command::Migrate {
            spec,
            from,
            to,
            new_sha,
            new_size,
            out,
        } => migrate(&catalog, &spec, &from, &to, &new_sha, new_size, &out),
    }
}

/// Loads a module and runs its constructors, which is what populates the
/// embind registry.
fn started(catalog: &Catalog, target: &str, opts: &EngineOpts) -> Result<Runtime> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let mut runtime = Runtime::instantiate(&bytes)?;
    if opts.threads {
        runtime.set_thread_policy(ThreadPolicy::Spawn);
    }
    runtime.run_ctors()?;
    if opts.log {
        // 1 MiB: large enough that startup does not wrap it, which would make
        // the transcript unreadable.
        runtime.attach_log_ring(1 << 20)?;
    }
    Ok(runtime)
}

/// Prints whatever the module logged, when a ring was attached.
fn print_engine_log(runtime: &Runtime, opts: &EngineOpts) {
    if !opts.log {
        return;
    }
    let lines = runtime.engine_log();
    if lines.is_empty() {
        return;
    }
    eprintln!("--- module log ({} lines) ---", lines.len());
    for line in lines {
        eprintln!("  {line}");
    }
}

/// Runs a WASI module against an in-memory filesystem.
fn run_wasi(
    catalog: &Catalog,
    target: &str,
    args: &[String],
    files: &[String],
    outputs: &[String],
    opts: &EngineOpts,
) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let mut runtime = Runtime::instantiate(&bytes)?;
    if opts.threads {
        runtime.set_thread_policy(ThreadPolicy::Spawn);
    }
    runtime.set_args(args);

    for spec in files {
        let (guest, host) = split_mapping(spec);
        let contents =
            std::fs::read(&host).with_context(|| format!("reading input file {host}"))?;
        runtime.add_file(&guest, contents);
    }
    if opts.log {
        runtime.attach_log_ring(1 << 20).ok();
    }

    let code = runtime.run_main()?;
    print!("{}", runtime.wasi().stdout_text());
    eprint!("{}", runtime.wasi().stderr_text());

    for spec in outputs {
        let (guest, host) = split_mapping(spec);
        let contents = runtime
            .wasi()
            .file(&guest)
            .ok_or_else(|| anyhow::anyhow!("guest wrote no file `{guest}`"))?;
        std::fs::write(&host, contents).with_context(|| format!("writing {host}"))?;
        eprintln!("wrote {host} ({} bytes)", contents.len());
    }

    print_engine_log(&runtime, opts);
    if code != 0 {
        eprintln!("exit: {code}");
    }
    std::process::exit(code);
}

/// Runs a derivation spec and reports what resolved and what was written.
fn derive(catalog: &Catalog, spec: &str, out: &str) -> Result<()> {
    use std::path::Path;

    let spec_path = Path::new(spec);
    let out_dir = Path::new(out);

    // Read the pin first so a spec naming an absent module fails before any
    // directory is created.
    let raw: serde_json::Value = serde_json::from_slice(
        &std::fs::read(spec_path).with_context(|| format!("reading {spec}"))?,
    )
    .with_context(|| format!("parsing {spec}"))?;
    let id = raw
        .get("module")
        .and_then(|module| module.get("id"))
        .and_then(|id| id.as_str())
        .with_context(|| format!("{spec} names no module.id"))?;

    let module = catalog.resolve(id)?;
    let manifest = oracle_core::derive::run_spec(spec_path, &module.path, out_dir)?;

    println!(
        "{} @ {} ({} bytes)\n",
        manifest.module.id,
        &manifest.module.sha256[..12],
        manifest.module.size
    );
    for (name, resolved) in &manifest.resolutions {
        println!(
            "  {name:<24} index {:<6} slots {:?}",
            resolved.index, resolved.slots
        );
    }
    println!();
    for output in &manifest.outputs {
        println!(
            "  wrote {:<40} {:>10} bytes  {}",
            output.file,
            output.bytes,
            &output.sha256[..12]
        );
    }
    println!("\n  manifest.json  spec {}", &manifest.spec_sha256[..12]);
    Ok(())
}

/// Carries a spec onto a new capture and reports what moved.
fn migrate(
    catalog: &Catalog,
    spec: &str,
    from: &str,
    to: &str,
    new_sha: &str,
    new_size: u64,
    out: &str,
) -> Result<()> {
    use std::path::Path;

    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(spec).with_context(|| format!("reading {spec}"))?)
            .with_context(|| format!("parsing {spec}"))?;
    let parsed: oracle_core::derive::Spec =
        serde_json::from_value(raw).with_context(|| format!("parsing {spec}"))?;

    let old = catalog.resolve(from)?;
    let new = catalog.resolve(to)?;
    let old_bytes =
        std::fs::read(&old.path).with_context(|| format!("reading {}", old.path.display()))?;
    let new_bytes =
        std::fs::read(&new.path).with_context(|| format!("reading {}", new.path.display()))?;

    let new_pin = oracle_core::derive::ModulePin {
        id: new.id.clone(),
        sha256: new_sha.to_owned(),
        size: new_size,
    };
    let (migrated, report) =
        oracle_core::migrate::migrate_spec(&parsed, &old_bytes, &new_bytes, &new_pin, &new.path)?;

    let text = serde_json::to_string_pretty(&migrated).context("encoding migrated spec")?;
    std::fs::write(Path::new(out), &text).with_context(|| format!("writing {out}"))?;

    println!(
        "{} ({} functions) -> {} ({}): {} carry one-to-one\n",
        old.id,
        report.coverage.old_functions,
        new.id,
        report.coverage.new_functions,
        report.coverage.carried
    );
    for (name, answer) in &report.selectors {
        match answer {
            oracle_core::migrate::Migrated::Carried {
                old_index,
                new_index,
                ..
            } => println!("  {name:<24} {old_index} -> {new_index}"),
            oracle_core::migrate::Migrated::Disambiguated {
                old_index,
                new_index,
                candidates,
            } => println!(
                "  {name:<24} {old_index} -> {new_index}  (anchor settled {candidates} sharers — review advised)"
            ),
            oracle_core::migrate::Migrated::NeedsHuman { old_index, reason } => {
                println!("  {name:<24} {old_index} -> STUCK: {reason}")
            }
        }
    }
    println!("\nwrote {out}");
    Ok(())
}

/// Splits `guest=host`, defaulting the guest name to the host file's own name.
fn split_mapping(spec: &str) -> (String, String) {
    match spec.split_once('=') {
        Some((guest, host)) => (guest.to_owned(), host.to_owned()),
        None => {
            let name = std::path::Path::new(spec)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(spec);
            (name.to_owned(), spec.to_owned())
        }
    }
}

fn embind(catalog: &Catalog, target: &str, full: bool, opts: &EngineOpts) -> Result<()> {
    let mut runtime = started(catalog, target, opts)?;
    let registry = runtime.embind();

    println!(
        "{} types, {} functions, {} classes, {} methods, {} properties\n",
        registry.types.len(),
        registry.functions.len(),
        registry.classes.len(),
        registry.method_count(),
        registry.property_count()
    );

    for function in &registry.functions {
        println!(
            "  {:<48} {}",
            function.name,
            registry.signature(&function.arg_types)
        );
    }

    if full {
        for class in registry.classes.values() {
            println!(
                "\n  class {} ({} methods, {} properties)",
                class.name,
                class.methods.len(),
                class.properties.len()
            );
            for method in &class.methods {
                println!(
                    "      {:<44} {}",
                    method.name,
                    registry.signature(&method.arg_types)
                );
            }
            // A property is a field, not a callable, so it is printed as one:
            // its type, and whether the module registered a setter for it.
            for property in &class.properties {
                let access = if property.setter_type.is_some() {
                    "get/set"
                } else {
                    "get"
                };
                println!(
                    "      {:<44} {} [{access}]",
                    property.name,
                    registry.type_name(property.field_type)
                );
            }
        }
    }
    print_engine_log(&runtime, opts);
    Ok(())
}

fn call(
    catalog: &Catalog,
    target: &str,
    function: &str,
    args: &[String],
    opts: &EngineOpts,
) -> Result<()> {
    let mut runtime = started(catalog, target, opts)?;
    runtime.clear_calls();

    let registry = runtime.embind();
    let declared = registry
        .functions
        .iter()
        .find(|candidate| candidate.name == function)
        .ok_or_else(|| anyhow::anyhow!("no embind function `{function}`"))?;

    // Skip index 0: that slot holds the return type.
    let param_types: Vec<u32> = declared.arg_types.iter().skip(1).copied().collect();
    if args.len() != param_types.len() {
        anyhow::bail!(
            "`{function}` {} takes {} argument(s), got {}",
            registry.signature(&declared.arg_types),
            param_types.len(),
            args.len()
        );
    }

    let values: Vec<Value> = args
        .iter()
        .zip(&param_types)
        .enumerate()
        .map(|(position, (arg, type_id))| {
            let type_name = registry.type_name(*type_id);
            parse_arg(arg, &type_name).with_context(|| {
                format!(
                    "argument {} of `{function}` (declared {type_name})",
                    position + 1
                )
            })
        })
        .collect::<Result<_>>()?;

    let result = runtime.call_embind(function, &values)?;
    println!("{result:?}");

    for line in runtime.logs() {
        eprintln!("log: {line}");
    }
    print_engine_log(&runtime, opts);
    Ok(())
}

/// Parses one command-line argument according to its registered C++ type.
///
/// A malformed value is an error, not a default. `yse` used to become `false`
/// and `1.0e` used to become `0.0`, so the guest ran on input the caller never
/// gave it and answered plausibly about something else — which from an oracle
/// is the worst available outcome.
///
/// The untyped fall-through is different and stays: a type this front end does
/// not model has no spelling to be wrong about, so a value that parses as an
/// integer is passed as one and anything else is passed as text.
fn parse_arg(raw: &str, type_name: &str) -> Result<Value> {
    Ok(match type_name {
        "bool" => match raw {
            "true" | "1" | "yes" => Value::Bool(true),
            "false" | "0" | "no" => Value::Bool(false),
            _ => anyhow::bail!("`{raw}` is not a bool; use true/false, 1/0 or yes/no"),
        },
        "float" | "double" => Value::Double(
            raw.parse()
                .with_context(|| format!("`{raw}` is not a number"))?,
        ),
        "std::string" => Value::Str(raw.to_owned()),
        _ => match raw.parse::<i64>() {
            Ok(value) => Value::Int(value),
            Err(_) => Value::Str(raw.to_owned()),
        },
    })
}

fn xref(catalog: &Catalog, target: &str, text: &str) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let found = oracle_core::abi::find_string_refs(&bytes, text)?;
    if found.is_empty() {
        println!("no string containing {text:?} in {}", module.id);
        return Ok(());
    }

    for entry in &found {
        println!("{:#x}  {:?}", entry.address, entry.text);
        if entry.referenced_by.is_empty() {
            if entry.pointer_slots.is_empty() {
                println!("    nothing loads this address and no pointer to it exists");
                continue;
            }
            // Reached through a table. The slot's own address is what the
            // emitting code indexes, so that is the next thing to look for.
            for slot in &entry.pointer_slots {
                println!("    pointer at {slot:#x}  (try: oracle xref-addr {target} {slot:#x})");
            }
            continue;
        }
        for func in &entry.referenced_by {
            println!("    function #{func}   (oracle abi {target} --index {func})");
        }
    }
    Ok(())
}

fn xref_addr(catalog: &Catalog, target: &str, address: &str, window: u32) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let address = match address.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16)?,
        None => address.parse()?,
    };

    let found = oracle_core::abi::find_address_refs(&bytes, address, window)?;
    if found.is_empty() {
        println!("nothing within {window} bytes before {address:#x} is loaded as a constant");
        return Ok(());
    }

    for entry in found.iter().take(20) {
        let position = if entry.offset == 0 {
            "loads it directly".to_owned()
        } else {
            format!("holds {:#x}, index {}", entry.loaded, entry.offset / 4)
        };
        println!(
            "function #{:<6} {position}   (oracle abi {target} --index {})",
            entry.function, entry.function
        );
    }
    if found.len() > 20 {
        println!("... and {} more", found.len() - 20);
    }
    Ok(())
}

fn enum_table(catalog: &Catalog, target: &str, base: &str, count: u32) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let base = match base.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16)?,
        None => base.parse()?,
    };

    for (index, entry) in oracle_core::abi::read_string_table(&bytes, base, count)?
        .into_iter()
        .enumerate()
    {
        match entry {
            Some(text) => println!("{index:>4}  {text}"),
            // Printed rather than skipped: a gap is where the table ends, and
            // silently closing it would shift every index after it.
            None => println!("{index:>4}  -"),
        }
    }
    Ok(())
}

fn constant_users(catalog: &Catalog, target: &str, value: i32) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let found = oracle_core::abi::find_constant_users(&bytes, value)?;
    if found.is_empty() {
        println!("no function loads {value}");
        return Ok(());
    }
    println!("{} function(s) load {value}:", found.len());
    for (function, uses) in found.iter().take(40) {
        println!(
            "  function #{function:<6} {uses} use(s)   (oracle abi {target} --index {function})"
        );
    }
    if found.len() > 40 {
        println!("  ... and {} more", found.len() - 40);
    }
    Ok(())
}

fn callers(catalog: &Catalog, target: &str, index: u32) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let found = oracle_core::abi::find_callers(&bytes, index)?;
    if found.is_empty() {
        println!("nothing calls #{index} directly (it may be reached through the table)");
        return Ok(());
    }
    for caller in &found {
        println!("function #{caller}   (oracle abi {target} --index {caller})");
    }
    Ok(())
}

fn abi(
    catalog: &Catalog,
    target: &str,
    filter: Option<&str>,
    slot: Option<u32>,
    index: Option<u32>,
    body: usize,
) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;
    oracle_core::abi::set_body_limit(body);

    if let Some(index) = index {
        match oracle_core::abi::infer_index(&bytes, index)? {
            Some(abi) => print!("{}", abi.describe()),
            None => println!("no function #{index} in this module"),
        }
        return Ok(());
    }

    if let Some(slot) = slot {
        match oracle_core::abi::infer_table_slot(&bytes, slot)? {
            Some(function) => print!("{}", function.describe()),
            None => println!("table slot {slot} holds no function"),
        }
        return Ok(());
    }

    let inferred = oracle_core::abi::infer(&bytes, |name| {
        filter.is_none_or(|needle| name.contains(needle))
    })?;

    if inferred.is_empty() {
        println!("no exported functions matched");
        return Ok(());
    }
    for function in &inferred {
        print!("{}", function.describe());
        if function.is_trampoline() {
            println!(
                "    ^ trampoline: the real arguments belong to its callee. Read the slot \n\
                 \x20     from arg0 at runtime, then `oracle abi {target} --slot <n>`."
            );
        }
    }
    Ok(())
}

fn strings(catalog: &Catalog, target: &str, min: usize) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;
    let report = oracle_core::data::extract(&bytes, min)?;

    println!(
        "{}: {} data segments, {}\n",
        module.id,
        report.segments.len(),
        human_size(report.total_bytes as u64)
    );
    for string in &report.strings {
        println!(
            "  [{:>3}+{:<6}] {}",
            string.segment, string.offset, string.value
        );
    }
    Ok(())
}

fn list(catalog: &Catalog) -> Result<()> {
    println!("{}\n", catalog.dir().display());
    for module in catalog.modules() {
        println!("  {:<16} {:>10}", module.id, human_size(module.size));
    }
    Ok(())
}

fn inspect(catalog: &Catalog, target: &str, full: bool) -> Result<()> {
    let module = catalog.resolve(target)?;
    let report = Inspection::run(&module)?;

    println!("{} ({})", report.id, human_size(report.size));
    println!("{}\n", report.path.display());

    for requirement in &report.requires {
        println!("  requires: {}\n", requirement.advice());
    }

    println!("  sections");
    for section in &report.sections {
        let detail = match (section.count, section.bytes) {
            (Some(count), Some(bytes)) => format!("{count} entries, {}", human_size(bytes as u64)),
            (Some(count), None) => format!("{count} entries"),
            (None, Some(bytes)) => human_size(bytes as u64),
            (None, None) => String::new(),
        };
        println!("    {:<24} {detail}", section.name);
    }

    println!(
        "\n  name section: {}",
        if report.toolchain.has_name_section {
            "present (internal names recoverable)"
        } else {
            "stripped"
        }
    );
    if !report.toolchain.producers.is_empty() {
        println!("  producers: {}", report.toolchain.producers.join(" "));
    }

    println!("\n  imports: {} total", report.imports.len());
    for (module_name, count) in report.import_modules() {
        println!("    {module_name:<28} {count}");
    }

    println!("\n  exports: {} total", report.exports.len());
    let mut funcs = 0;
    let mut others = Vec::new();
    for export in &report.exports {
        match &export.kind {
            EntryKind::Func(_) => funcs += 1,
            other => others.push(format!("{} ({other})", export.name)),
        }
    }
    println!("    {funcs} functions");
    for other in &others {
        println!("    {other}");
    }

    if full {
        println!("\n  --- imports ---");
        for import in &report.imports {
            println!("    {}::{}  {}", import.module, import.name, import.kind);
        }
        println!("\n  --- exports ---");
        for export in &report.exports {
            println!("    {:<40} {}", export.name, export.kind);
        }
    } else if funcs > 0 {
        println!("\n  exported functions (first 40, use --full for all)");
        for export in report
            .exports
            .iter()
            .filter(|export| matches!(export.kind, EntryKind::Func(_)))
            .take(40)
        {
            println!("    {:<40} {}", export.name, export.kind);
        }
    }

    Ok(())
}

/// Brings a module up and reports what its startup actually did.
fn instantiate(catalog: &Catalog, target: &str, threads: bool) -> Result<()> {
    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)?;

    let mut runtime = Runtime::instantiate(&bytes)?;
    if threads {
        runtime.set_thread_policy(ThreadPolicy::Spawn);
    }
    println!("{} ({})\n", module.id, human_size(module.size));

    if !runtime.unstubbable.is_empty() {
        println!("  unstubbable imports: {}", runtime.unstubbable.len());
        for symbol in &runtime.unstubbable {
            println!("    {symbol}");
        }
        println!();
    }

    match runtime.run_ctors() {
        Ok(()) => println!("  constructors: ok"),
        Err(error) => println!("  constructors: {}", first_line(&format!("{error:#}"))),
    }

    let settled = runtime.quiesce(std::time::Duration::from_secs(10));
    println!(
        "  threads:      {} live, quiesced: {settled}",
        runtime.live_threads()
    );

    println!(
        "\n  host calls during startup: {}",
        runtime.state().total_calls()
    );
    for (symbol, count) in runtime.state().hot_calls().iter().take(12) {
        println!("    {count:>10}  {symbol}");
    }

    let logs = runtime.logs();
    if !logs.is_empty() {
        println!("\n  host log ({} lines)", logs.len());
        for line in logs.iter().take(10) {
            println!("    {}", first_line(line));
        }
    }
    Ok(())
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Writes an instrumented copy and prints where the markers went.
fn instrument(catalog: &Catalog, target: &str, out: &str, marks: &MarkOpts) -> Result<()> {
    use oracle_core::patch::{self, Plan};

    let mut value_entry = Vec::new();
    for spec in &marks.value {
        let (func, local) = spec
            .split_once(':')
            .with_context(|| format!("--value wants FUNC:LOCAL, got `{spec}`"))?;
        value_entry.push((
            func.parse()
                .with_context(|| format!("function in `{spec}`"))?,
            local
                .parse()
                .with_context(|| format!("local in `{spec}`"))?,
        ));
    }

    let plan = Plan {
        entry: marks.entry.clone(),
        value_entry,
        value_at: Vec::new(),
        before_calls: marks.calls_in.iter().map(|func| (*func, None)).collect(),
        at_returns: marks.returns_in.clone(),
        id_base: patch::DEFAULT_ID_BASE,
        sink: marks.sink.clone(),
    };

    if plan.entry.is_empty()
        && plan.value_entry.is_empty()
        && plan.before_calls.is_empty()
        && plan.at_returns.is_empty()
    {
        anyhow::bail!(
            "nothing to instrument: pass at least one of --entry, --calls-in, --value or --returns-in"
        );
    }

    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)
        .with_context(|| format!("reading {}", module.path.display()))?;

    let (rewritten, map) = patch::instrument(&bytes, &plan)?;
    std::fs::write(out, &rewritten).with_context(|| format!("writing {out}"))?;

    println!(
        "{} -> {} ({} bytes, was {}), markers call {}",
        module.id,
        out,
        rewritten.len(),
        bytes.len(),
        map.via_symbol
    );
    for marker in &map.markers {
        println!("  {:>7}  {:<12} {}", marker.id, marker.kind, marker.detail);
    }
    Ok(())
}

/// Writes a copy with instructions replaced.
fn patch_module(catalog: &Catalog, target: &str, out: &str, replace: &[String]) -> Result<()> {
    use oracle_core::patch;

    if replace.is_empty() {
        anyhow::bail!("nothing to do: pass at least one --replace FUNC:AT:COUNT:SPEC");
    }

    let edits: Vec<_> = replace
        .iter()
        .map(|spec| patch::parse_replace(spec))
        .collect::<Result<_>>()?;

    let module = catalog.resolve(target)?;
    let bytes = std::fs::read(&module.path)
        .with_context(|| format!("reading {}", module.path.display()))?;

    let rewritten = patch::replace(&bytes, &edits)?;
    std::fs::write(out, &rewritten).with_context(|| format!("writing {out}"))?;

    println!(
        "{} -> {} ({} bytes, was {}), {} replacement(s)",
        module.id,
        out,
        rewritten.len(),
        bytes.len(),
        edits.len()
    );
    Ok(())
}

/// Carries indices between captures, reporting coverage first.
fn carry(catalog: &Catalog, old: &str, new: &str, indices: &[u32]) -> Result<()> {
    use oracle_core::carry::{Carried, carry_indices};

    let old = catalog.resolve(old)?;
    let new = catalog.resolve(new)?;
    let (coverage, answers) = carry_indices(&old.path, &new.path, indices)?;

    // Coverage first: it says whether this bump is a renumbering or a rewrite,
    // and a low number means the per-index answers below deserve less trust.
    println!(
        "{} ({} functions) -> {} ({}): {} carry one-to-one",
        old.id, coverage.old_functions, new.id, coverage.new_functions, coverage.carried
    );

    for (index, answer) in answers {
        match answer {
            Carried::One(found) => println!("  {index} -> {found}"),
            Carried::Ambiguous { old, new } => println!(
                "  {index} -> ambiguous: {old} old / {} new share this shape {:?}",
                new.len(),
                &new[..new.len().min(8)]
            ),
            Carried::Changed => {
                println!("  {index} -> changed; no mechanical route, re-derive it")
            }
            Carried::NotFingerprintable => {
                println!("  {index} -> too short to fingerprint, or imported")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A malformed typed argument is an error, not a default.
    ///
    /// Coercing `yse` to `false` and `1.0e` to `0.0` ran the guest on input the
    /// caller never gave it, and the answer that came back was plausible and
    /// about something else — which from an oracle is worse than no answer.
    #[test]
    fn a_malformed_typed_argument_is_refused() {
        assert_eq!(parse_arg("true", "bool").unwrap(), Value::Bool(true));
        assert_eq!(parse_arg("0", "bool").unwrap(), Value::Bool(false));
        assert_eq!(parse_arg("-1.5", "double").unwrap(), Value::Double(-1.5));

        for (raw, type_name) in [("yse", "bool"), ("1.0e", "double"), ("", "float")] {
            let error = parse_arg(raw, type_name).unwrap_err().to_string();
            assert!(
                error.contains(raw) || raw.is_empty(),
                "the refusal should quote what it could not parse: {error}"
            );
        }
    }

    /// The untyped fall-through is deliberate and stays: a type this front end
    /// does not model has no spelling to be wrong about.
    #[test]
    fn an_unmodelled_type_still_takes_an_integer_or_a_string() {
        assert_eq!(parse_arg("42", "MyEnum").unwrap(), Value::Int(42));
        assert_eq!(
            parse_arg("hello", "MyEnum").unwrap(),
            Value::Str("hello".to_owned())
        );
    }
}
