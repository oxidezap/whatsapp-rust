//! Static inspection of a captured module.
//!
//! Everything here is read with `wasmparser` and nothing is compiled. Inspection
//! answers questions about a file — what it imports, what it exports, what built
//! it — and compiling megabytes of machine code to answer them costs seconds per
//! module for information already present in the bytes.
//!
//! Function signatures therefore have to be resolved by hand: the type section
//! holds the signatures, the import and function sections say which one each
//! function uses, and the export section refers to functions by an index space
//! that starts with the imported ones. That bookkeeping is the price of not
//! compiling.

use std::fmt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use wasmparser::{Parser, Payload, TypeRef};

use crate::catalog::CapturedModule;

/// What an export or import is, with the detail worth printing next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// A function, carrying its signature rendered by hand from the type
    /// section — no runtime is involved in resolving it.
    Func(String),
    /// A memory. `shared` is the one that decides whether the module needs the
    /// threads proposal, which is why it is reported rather than inferred from
    /// a failed instantiation.
    Memory {
        /// Initial size, in pages.
        min: u64,
        /// Declared maximum, in pages, if it has one.
        max: Option<u64>,
        /// Whether it is a shared memory.
        shared: bool,
    },
    /// A table.
    Table {
        /// Initial size, in elements.
        min: u64,
        /// Declared maximum, in elements, if it has one.
        max: Option<u64>,
    },
    /// A global.
    Global,
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Func(signature) => write!(f, "func {signature}"),
            Self::Memory { min, max, shared } => {
                let shared = if *shared { " shared" } else { "" };
                write!(f, "memory{shared} {min}..{}", DisplayMax(*max))
            }
            Self::Table { min, max } => write!(f, "table {min}..{}", DisplayMax(*max)),
            Self::Global => f.write_str("global"),
        }
    }
}

struct DisplayMax(Option<u64>);

impl fmt::Display for DisplayMax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(max) => write!(f, "{max}"),
            None => f.write_str("∞"),
        }
    }
}

/// One export.
#[derive(Debug, Clone)]
pub struct ExportEntry {
    /// The name the module publishes it under.
    pub name: String,
    /// What it is.
    pub kind: EntryKind,
}

/// One import, which is what says which host environment a module wants.
#[derive(Debug, Clone)]
pub struct ImportEntry {
    /// The import module, e.g. `env` or `wasi_snapshot_preview1`.
    pub module: String,
    /// The name within that module.
    pub name: String,
    /// What it is.
    pub kind: EntryKind,
}

/// One section, summarised.
#[derive(Debug, Clone)]
pub struct SectionEntry {
    /// Section name, or `custom:<name>` for a custom section.
    pub name: String,
    /// Element count for indexed sections; `None` for custom sections, where
    /// `bytes` is the meaningful figure.
    pub count: Option<u32>,
    /// Size in bytes, where it is worth reporting.
    pub bytes: Option<usize>,
}

/// What produced the module, as claimed by the `producers` custom section.
#[derive(Debug, Clone, Default)]
pub struct Toolchain {
    /// What the `producers` section claims, verbatim. A claim, not a fact:
    /// nothing stops a module from carrying someone else's.
    pub producers: Vec<String>,
    /// Whether a `name` custom section survived, i.e. whether internal function
    /// names are recoverable.
    pub has_name_section: bool,
}

/// A wasm proposal a module depends on, which decides what can run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// The module declares a shared memory, so it expects threads. A runtime
    /// without them cannot load it at all.
    Threads,
}

impl Requirement {
    /// One line saying what the requirement means for running the module.
    #[must_use]
    pub fn advice(self) -> &'static str {
        match self {
            Self::Threads => {
                "module declares a shared memory: it needs the threads proposal, and \
                 ThreadPolicy::Spawn to make progress past its own initialisation"
            }
        }
    }
}

/// Everything static inspection reports about a module.
///
/// Built by streaming the module with `wasmparser`: no bodies are decoded and
/// nothing is compiled, which is what keeps this under 10 ms on a 10 MB file.
#[derive(Debug, Clone)]
pub struct Inspection {
    /// The module's catalogue id.
    pub id: String,
    /// Where it was read from.
    pub path: PathBuf,
    /// Its size in bytes.
    pub size: u64,
    /// Every section, in order.
    pub sections: Vec<SectionEntry>,
    /// What the `producers` section claims, and whether names survived.
    pub toolchain: Toolchain,
    /// Everything the module exports.
    pub exports: Vec<ExportEntry>,
    /// Everything it imports, which names the host environment it wants.
    pub imports: Vec<ImportEntry>,
    /// Proposals the module needs, inferred from what it declares.
    pub requires: Vec<Requirement>,
}

impl Inspection {
    /// Reads a catalogued module and reports what it declares.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not a valid module.
    pub fn run(module: &CapturedModule) -> Result<Self> {
        let bytes = std::fs::read(&module.path)
            .with_context(|| format!("reading {}", module.path.display()))?;
        let parsed = parse(&bytes)?;

        Ok(Self {
            id: module.id.clone(),
            path: module.path.clone(),
            size: module.size,
            sections: parsed.sections,
            toolchain: parsed.toolchain,
            exports: parsed.exports,
            imports: parsed.imports,
            requires: parsed.requires,
        })
    }

    /// Import modules and how many symbols each one contributes, most first.
    /// This is the shape of the host environment that has to be provided before
    /// the module will run.
    pub fn import_modules(&self) -> Vec<(&str, usize)> {
        let mut grouped: Vec<(&str, usize)> = Vec::new();
        for import in &self.imports {
            match grouped.iter_mut().find(|(name, _)| *name == import.module) {
                Some((_, count)) => *count += 1,
                None => grouped.push((import.module.as_str(), 1)),
            }
        }
        grouped.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        grouped
    }
}

#[derive(Default)]
struct Parsed {
    sections: Vec<SectionEntry>,
    toolchain: Toolchain,
    exports: Vec<ExportEntry>,
    imports: Vec<ImportEntry>,
    requires: Vec<Requirement>,
}

/// Reads a module in one pass.
fn parse(bytes: &[u8]) -> Result<Parsed> {
    let mut out = Parsed::default();

    // Signatures, and the type index of every function in index order. Imported
    // functions occupy the start of that space, which is what makes an export's
    // index resolvable.
    let mut signatures: Vec<String> = Vec::new();
    let mut func_types: Vec<u32> = Vec::new();
    // Memories and tables in *index-space* order: the imported ones first, then
    // the locally defined ones, exactly as functions are. An export names an
    // index into that combined space, so keeping only the local ones made every
    // table export report the first local table's limits — and a re-exported
    // imported table, in a module with no local one, report as a global.
    let mut memories: Vec<(u64, Option<u64>, bool)> = Vec::new();
    let mut tables: Vec<(u64, Option<u64>)> = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.context("parsing wasm sections")? {
            Payload::TypeSection(reader) => {
                let count = reader.count();
                for group in reader {
                    for sub in group.context("reading type")?.into_types() {
                        signatures.push(format_composite(&sub.composite_type));
                    }
                }
                out.sections.push(counted("type", count));
            }

            Payload::ImportSection(reader) => {
                let count = reader.count();
                // `into_imports` flattens the compact-import groups that newer
                // encoders emit; iterating the section directly yields groups,
                // not individual imports.
                for import in reader.into_imports() {
                    let import = import.context("reading import")?;
                    let kind = match import.ty {
                        // `FuncExact` is the exact-typed form of a function
                        // import; both carry a type index.
                        TypeRef::Func(index) | TypeRef::FuncExact(index) => {
                            func_types.push(index);
                            EntryKind::Func(signature_of(&signatures, index)?)
                        }
                        TypeRef::Memory(ty) => {
                            memories.push((ty.initial, ty.maximum, ty.shared));
                            EntryKind::Memory {
                                min: ty.initial,
                                max: ty.maximum,
                                shared: ty.shared,
                            }
                        }
                        TypeRef::Table(ty) => {
                            tables.push((ty.initial, ty.maximum));
                            EntryKind::Table {
                                min: ty.initial,
                                max: ty.maximum,
                            }
                        }
                        TypeRef::Global(_) | TypeRef::Tag(_) => EntryKind::Global,
                    };
                    if let EntryKind::Memory { shared: true, .. } = kind {
                        note_requirement(&mut out.requires, Requirement::Threads);
                    }
                    out.imports.push(ImportEntry {
                        module: import.module.to_owned(),
                        name: import.name.to_owned(),
                        kind,
                    });
                }
                out.sections.push(counted("import", count));
            }

            Payload::FunctionSection(reader) => {
                let count = reader.count();
                for index in reader {
                    func_types.push(index.context("reading function")?);
                }
                out.sections.push(counted("function", count));
            }

            Payload::MemorySection(reader) => {
                let count = reader.count();
                for memory in reader {
                    let memory = memory.context("reading memory")?;
                    if memory.shared {
                        note_requirement(&mut out.requires, Requirement::Threads);
                    }
                    memories.push((memory.initial, memory.maximum, memory.shared));
                }
                out.sections.push(counted("memory", count));
            }

            Payload::TableSection(reader) => {
                let count = reader.count();
                for table in reader {
                    let table = table.context("reading table")?;
                    tables.push((table.ty.initial, table.ty.maximum));
                }
                out.sections.push(counted("table", count));
            }

            Payload::ExportSection(reader) => {
                let count = reader.count();
                for export in reader {
                    let export = export.context("reading export")?;
                    let index = export.index as usize;
                    let kind = match export.kind {
                        wasmparser::ExternalKind::Func => {
                            let type_index = func_types.get(index).with_context(|| {
                                format!(
                                    "function export {:?} references missing function index {index}",
                                    export.name
                                )
                            })?;
                            EntryKind::Func(signature_of(&signatures, *type_index)?)
                        }
                        // `memories` and `tables` are already in index-space
                        // order, so the export's own index is the lookup.
                        wasmparser::ExternalKind::Memory => match memories.get(index) {
                            Some((min, max, shared)) => EntryKind::Memory {
                                min: *min,
                                max: *max,
                                shared: *shared,
                            },
                            None => EntryKind::Global,
                        },
                        wasmparser::ExternalKind::Table => match tables.get(index) {
                            Some((min, max)) => EntryKind::Table {
                                min: *min,
                                max: *max,
                            },
                            None => EntryKind::Global,
                        },
                        _ => EntryKind::Global,
                    };
                    out.exports.push(ExportEntry {
                        name: export.name.to_owned(),
                        kind,
                    });
                }
                out.sections.push(counted("export", count));
            }

            Payload::GlobalSection(reader) => out.sections.push(counted("global", reader.count())),
            Payload::ElementSection(reader) => {
                out.sections.push(counted("element", reader.count()))
            }
            Payload::DataSection(reader) => out.sections.push(counted("data", reader.count())),
            Payload::CodeSectionStart { count, size, .. } => out.sections.push(SectionEntry {
                name: "code".to_owned(),
                count: Some(count),
                bytes: Some(size as usize),
            }),

            Payload::CustomSection(reader) => {
                let name = reader.name();
                if name == "name" {
                    out.toolchain.has_name_section = true;
                }
                if name == "producers" {
                    out.toolchain.producers = producer_strings(reader.data());
                }
                out.sections.push(SectionEntry {
                    name: format!("custom:{name}"),
                    count: None,
                    bytes: Some(reader.data().len()),
                });
            }

            _ => {}
        }
    }

    out.exports.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn note_requirement(requires: &mut Vec<Requirement>, requirement: Requirement) {
    if !requires.contains(&requirement) {
        requires.push(requirement);
    }
}

fn signature_of(signatures: &[String], index: u32) -> Result<String> {
    signatures
        .get(index as usize)
        .cloned()
        .with_context(|| format!("missing function type index {index}"))
}

fn format_composite(composite: &wasmparser::CompositeType) -> String {
    let wasmparser::CompositeInnerType::Func(func) = &composite.inner else {
        return "(non-function type)".to_owned();
    };

    let params = func
        .params()
        .iter()
        .map(format_val)
        .collect::<Vec<_>>()
        .join(", ");
    let results = func
        .results()
        .iter()
        .map(format_val)
        .collect::<Vec<_>>()
        .join(", ");

    if results.is_empty() {
        format!("({params})")
    } else {
        format!("({params}) -> {results}")
    }
}

fn format_val(ty: &wasmparser::ValType) -> String {
    match ty {
        wasmparser::ValType::I32 => "i32".to_owned(),
        wasmparser::ValType::I64 => "i64".to_owned(),
        wasmparser::ValType::F32 => "f32".to_owned(),
        wasmparser::ValType::F64 => "f64".to_owned(),
        wasmparser::ValType::V128 => "v128".to_owned(),
        wasmparser::ValType::Ref(_) => "ref".to_owned(),
    }
}

fn counted(name: &str, count: u32) -> SectionEntry {
    SectionEntry {
        name: name.to_owned(),
        count: Some(count),
        bytes: None,
    }
}

/// The producers section is a nested name/value-list structure. Rather than
/// depend on a decoder whose API churns between wasmparser releases, pull the
/// printable runs out of it: for identifying a toolchain that is enough, and it
/// degrades gracefully on a malformed section.
fn producer_strings(data: &[u8]) -> Vec<String> {
    const MIN_RUN: usize = 3;

    let mut found = Vec::new();
    let mut current = String::new();
    for &byte in data {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte as char);
        } else if current.len() >= MIN_RUN {
            found.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() >= MIN_RUN {
        found.push(current);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_encoder::{
        EntityType, ExportKind, ExportSection, ImportSection, MemorySection, MemoryType, Module,
        RefType, TableSection, TableType,
    };

    fn table_type(minimum: u64, maximum: Option<u64>) -> TableType {
        TableType {
            element_type: RefType::FUNCREF,
            table64: false,
            minimum,
            maximum,
            shared: false,
        }
    }

    fn memory_type(minimum: u64, shared: bool) -> MemoryType {
        MemoryType {
            minimum,
            maximum: None,
            memory64: false,
            shared,
            page_size_log2: None,
        }
    }

    /// An export names an index into the *whole* space, imports included, and
    /// each kind gets its own space. Reporting every table export from the
    /// first locally declared table made a two-table module lie about both, and
    /// a module whose only table is imported report it as a global.
    #[test]
    fn an_export_resolves_to_the_table_its_index_names() {
        let mut imports = ImportSection::new();
        imports.import("env", "table", EntityType::Table(table_type(7, Some(7))));
        imports.import("env", "memory", EntityType::Memory(memory_type(2, true)));

        let mut tables = TableSection::new();
        tables.table(table_type(11, None));
        tables.table(table_type(23, Some(99)));

        let mut memories = MemorySection::new();
        memories.memory(memory_type(5, false));

        let mut exports = ExportSection::new();
        // Index 0 is the imported table, 1 and 2 the local ones.
        exports.export("borrowed", ExportKind::Table, 0);
        exports.export("first", ExportKind::Table, 1);
        exports.export("second", ExportKind::Table, 2);
        // And the same for memories: 0 imported, 1 local.
        exports.export("shared_mem", ExportKind::Memory, 0);
        exports.export("own_mem", ExportKind::Memory, 1);

        let mut module = Module::new();
        module.section(&imports);
        module.section(&tables);
        module.section(&memories);
        module.section(&exports);

        let parsed = parse(&module.finish()).expect("parse");
        let kind = |name: &str| {
            parsed
                .exports
                .iter()
                .find(|export| export.name == name)
                .unwrap_or_else(|| panic!("no export `{name}`"))
                .kind
                .clone()
        };

        assert_eq!(
            kind("borrowed"),
            EntryKind::Table {
                min: 7,
                max: Some(7)
            },
            "a re-exported imported table is a table, not a global"
        );
        assert_eq!(kind("first"), EntryKind::Table { min: 11, max: None });
        assert_eq!(
            kind("second"),
            EntryKind::Table {
                min: 23,
                max: Some(99)
            },
            "the second local table must not report the first one's limits"
        );
        assert_eq!(
            kind("shared_mem"),
            EntryKind::Memory {
                min: 2,
                max: None,
                shared: true
            }
        );
        assert_eq!(
            kind("own_mem"),
            EntryKind::Memory {
                min: 5,
                max: None,
                shared: false
            }
        );
    }

    #[test]
    fn an_export_cannot_reference_a_missing_function() {
        let mut exports = ExportSection::new();
        exports.export("missing", ExportKind::Func, 7);
        let mut module = Module::new();
        module.section(&exports);

        let error = match parse(&module.finish()) {
            Ok(_) => panic!("invalid function export was accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("function index 7"), "{error:#}");
    }
}
