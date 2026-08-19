//! `Client`'s growable state must stay reachable from `memory_report()`.
//!
//! The gap this guards is the one that made per-session RAM unattributable: a
//! cache lands on `Client`, nobody adds it to the report, and the only symptom
//! is an RSS number no report explains. Scalars, locks and notifiers are left
//! alone, since they cost `size_of` and cannot drift. Collections can, so every
//! field whose type names one is either walked by the report or listed below
//! with the reason it is not.
//!
//! A field's own type expression is not the whole answer: a collection wrapped
//! in a newtype (`pending_device_sync: PendingDeviceSync`) names nothing
//! growable and used to pass unseen, which is how the biggest per-client
//! retention in the library — the inbound commit batch, which accumulates to
//! 4 MiB of decoded protos — stayed out of the report. So the scan also resolves
//! each field's crate-local types one level, aliases expanded, and looks at
//! *their* fields.
//!
//! One level, not transitively: `self_weak: Weak<Client>` makes the graph
//! reach every collection from every field, and a guard that flags everything
//! flags nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Type constructors that can retain an unbounded number of entries. A field
/// naming one of these anywhere in its type is a candidate for the report.
const GROWABLE: &[&str] = &[
    "BTreeMap",
    "Cache",
    "HashMap",
    "HashSet",
    "TypedCache",
    "Vec",
    "VecDeque",
];

/// Growable fields the report deliberately does not walk. Each entry states why
/// it is not a per-session memory question; a new cache does NOT belong here.
const EXEMPT: &[(&str, &str)] = &[
    (
        "offline_receipt_buffer",
        "drained at the end of every offline batch; bounded by one batch, and the \
         MessageInfo values are already counted where the batch owns them",
    ),
    (
        "message_retry_counts",
        "counter-only map bounded by the retry ceiling; entries are two integers",
    ),
    (
        "self_weak",
        "a `Weak<Client>` back to this same client, so its collections are the \
         ones every other field already accounts for",
    ),
    (
        "plugin_host",
        "walked by the feature-gated plugin section of the report (installed \
         plugins, tasks, subscriptions, endpoint queues), not by the common list",
    ),
    (
        "lifecycle",
        "callbacks and connection scopes registered once at build by the \
         extension host; fixed for the client's lifetime, not workload-driven",
    ),
    (
        "stanza_router",
        "one handler per protocol tag, populated by `create_stanza_router` at \
         construction and never written again",
    ),
    (
        "device_topology",
        "the changed-users log is a fixed-length ring (TOPOLOGY_LOG_CAPACITY); \
         overflow degrades memos to a recompute rather than retaining more",
    ),
    (
        "media_conn",
        "the host list from the most recent `<media_conn>` IQ — one server \
         response, replaced wholesale on refresh",
    ),
    (
        "ab_props",
        "server props are filtered against the compile-time `WATCHED` interest \
         set at parse time, so the map is bounded by that list and not by what \
         the server sends",
    ),
    (
        "pair_code_state",
        "one in-flight pairing attempt; its `Vec` is the server's pairing ref, \
         a handful of bytes, not a collection of entries",
    ),
];

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = manifest_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

/// Every ident appearing anywhere in a type expression, including inside
/// generic arguments (`Arc<Mutex<HashMap<..>>>` yields all four).
fn type_idents(ty: &syn::Type, out: &mut Vec<String>) {
    match ty {
        syn::Type::Path(path) => {
            for segment in &path.path.segments {
                out.push(segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            type_idents(inner, out);
                        }
                    }
                }
            }
        }
        syn::Type::Reference(r) => type_idents(&r.elem, out),
        syn::Type::Paren(p) => type_idents(&p.elem, out),
        syn::Type::Group(g) => type_idents(&g.elem, out),
        syn::Type::Tuple(t) => t.elems.iter().for_each(|e| type_idents(e, out)),
        syn::Type::Array(a) => type_idents(&a.elem, out),
        _ => {}
    }
}

/// Every `.rs` file under `src/`, so a type can be looked up wherever it is
/// defined rather than only where it is used.
fn crate_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            crate_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Type name -> the idents appearing in that type's own field types.
///
/// Structs and enums both, since either can hold a collection. Names are not
/// qualified by module: two same-named types would merge their fields, which
/// can only over-report (a candidate that then has to be reported or exempted),
/// never hide one.
fn crate_type_fields() -> (HashMap<String, Vec<String>>, HashMap<String, Vec<String>>) {
    fn collect(
        items: &[syn::Item],
        defs: &mut HashMap<String, Vec<String>>,
        aliases: &mut HashMap<String, Vec<String>>,
    ) {
        for item in items {
            let (name, field_types) = match item {
                syn::Item::Struct(item) => (
                    item.ident.to_string(),
                    item.fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>(),
                ),
                syn::Item::Enum(item) => (
                    item.ident.to_string(),
                    item.variants
                        .iter()
                        .flat_map(|v| v.fields.iter().map(|f| f.ty.clone()))
                        .collect(),
                ),
                syn::Item::Type(item) => {
                    // An alias is a spelling, not a type: `type Pending =
                    // HashMap<..>` must not let a field hide a map behind one.
                    let entry = aliases.entry(item.ident.to_string()).or_default();
                    type_idents(&item.ty, entry);
                    continue;
                }
                syn::Item::Mod(item) => {
                    if let Some((_, inner)) = &item.content {
                        collect(inner, defs, aliases);
                    }
                    continue;
                }
                _ => continue,
            };
            let entry = defs.entry(name).or_default();
            for ty in &field_types {
                type_idents(ty, entry);
            }
        }
    }

    let mut files = Vec::new();
    crate_sources(&manifest_path("src"), &mut files);
    // `wacore` too: `Client` holds several of its types, and a collection behind
    // one of those is no less a per-session cost for living in another crate.
    // `AppStateProcessor`'s unbounded key cache is the one that proved it.
    crate_sources(&manifest_path("wacore/src"), &mut files);
    let mut defs = HashMap::new();
    let mut aliases = HashMap::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("read {file:?}: {e}"));
        let parsed = syn::parse_file(&text).unwrap_or_else(|e| panic!("parse {file:?}: {e}"));
        collect(&parsed.items, &mut defs, &mut aliases);
    }

    // Substitute aliases so the lookup below stays one level deep: aliases are
    // resolved among themselves to a fixed point, then applied to every type's
    // field idents.
    flatten_alias_chains(&mut aliases);
    for idents in defs.values_mut() {
        *idents = resolve_aliases(idents, &aliases);
    }
    // The alias map is returned too: a `Client` field can name one directly
    // (`group_cache: OnceLock<Arc<GroupCache>>` over `type GroupCache =
    // TypedCache<..>`), and expanding only inside type definitions would leave
    // that field invisible — removing it from the report would keep this green.
    (defs, aliases)
}

/// `idents` with every alias replaced, one substitution deep, by what it stands
/// for. Chains are handled by [`flatten_alias_chains`] having already collapsed
/// the map, so one pass here is enough.
fn resolve_aliases(idents: &[String], aliases: &HashMap<String, Vec<String>>) -> Vec<String> {
    idents
        .iter()
        .flat_map(|ident| match aliases.get(ident) {
            // A self-referential spelling (`type Cache = Cache<..>`) would
            // otherwise expand forever without adding anything.
            Some(target) if target != std::slice::from_ref(ident) => target.clone(),
            _ => vec![ident.clone()],
        })
        .collect()
}

/// Collapse `A -> B -> … -> Vec` so every alias maps directly to what it
/// bottoms out in.
///
/// Iterates to a fixed point rather than a fixed number of rounds: a chain
/// longer than the loop would otherwise leave its tail unresolved, and the
/// resulting miss looks exactly like "this field holds no collection".
///
/// Each round dedups, which is what makes the iteration terminate rather than
/// blow up. Without it an alias naming another one twice (`type Pair = (Foo,
/// Foo)`) doubles its ident list every round — with the whole of `wacore` in the
/// map that is an OOM, not a slow test. Since the question asked of the result
/// is only *which* idents are reachable, multiplicity carries no information,
/// so dropping it costs nothing and bounds each list by the number of distinct
/// idents in the tree.
fn flatten_alias_chains(aliases: &mut HashMap<String, Vec<String>>) {
    fn dedup(mut idents: Vec<String>) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        idents.retain(|ident| seen.insert(ident.clone()));
        idents
    }

    for idents in aliases.values_mut() {
        *idents = dedup(std::mem::take(idents));
    }
    loop {
        let snapshot = aliases.clone();
        let mut changed = false;
        for idents in aliases.values_mut() {
            let expanded = dedup(resolve_aliases(idents, &snapshot));
            if *idents != expanded {
                *idents = expanded;
                changed = true;
            }
        }
        if !changed {
            return;
        }
    }
}

/// How a field reaches a growable: directly in its own type, or through one
/// crate-local type it names. Returned for the failure message, so a new
/// candidate says which collection put it on the list.
fn growable_path(idents: &[String], defs: &HashMap<String, Vec<String>>) -> Option<String> {
    if let Some(direct) = idents.iter().find(|i| GROWABLE.contains(&i.as_str())) {
        return Some(direct.clone());
    }
    for ident in idents {
        let Some(fields) = defs.get(ident) else {
            continue;
        };
        if let Some(inner) = fields.iter().find(|i| GROWABLE.contains(&i.as_str())) {
            return Some(format!("{ident}::{inner}"));
        }
    }
    None
}

/// Whether `name` occurs in `text` as a whole identifier. A plain substring
/// search would let a field called `cache` pass on any mention of `group_cache`;
/// requiring the boundaries keeps that from counting.
fn mentions(text: &str, name: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !ident(c));
        let after = text[at + name.len()..]
            .chars()
            .next()
            .is_none_or(|c| !ident(c));
        before && after
    })
}

/// The body of the function whose signature starts with `signature`,
/// brace-matched with line comments stripped first, or `None` when the file has
/// no such function.
fn fn_body(text: &str, signature: &str) -> Option<String> {
    let stripped: String = text
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let at = stripped.find(signature)?;
    let open = at + stripped[at..].find('{')?;
    let mut depth = 0usize;
    for (offset, ch) in stripped[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(stripped[open..=open + offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// The body of `memory_report()`, brace-matched from its signature with line
/// comments stripped first. Searching all of `accessors.rs` would let an
/// unrelated getter, or a field named in a comment, stand in for the report
/// actually walking it.
fn memory_report_body(text: &str) -> String {
    let stripped: String = text
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    const SIGNATURE: &str = "pub async fn memory_report(";
    let at = stripped
        .find(SIGNATURE)
        .expect("`memory_report` in src/client/accessors.rs");
    let open = at + stripped[at..].find('{').expect("a body for memory_report");

    let mut depth = 0usize;
    for (offset, ch) in stripped[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return stripped[open..=open + offset].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces after `memory_report`");
}

fn client_struct(text: &str) -> syn::ItemStruct {
    let file = syn::parse_file(text).expect("parse src/client.rs");
    file.items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "Client" => Some(item),
            _ => None,
        })
        .expect("`struct Client` in src/client.rs")
}

#[test]
fn every_growable_client_field_reaches_the_memory_report() {
    let client = client_struct(&read("src/client.rs"));
    let report = memory_report_body(&read("src/client/accessors.rs"));
    let (defs, aliases) = crate_type_fields();

    let mut missing = Vec::new();
    for field in &client.fields {
        let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
            continue;
        };
        let mut idents = Vec::new();
        type_idents(&field.ty, &mut idents);
        let idents = resolve_aliases(&idents, &aliases);
        let Some(via) = growable_path(&idents, &defs) else {
            continue;
        };
        if EXEMPT.iter().any(|(exempt, _)| *exempt == name) {
            continue;
        }
        if !mentions(&report, &name) {
            missing.push(format!("  {name}: {} (via {via})", idents.join("<")));
        }
    }

    assert!(
        missing.is_empty(),
        "these `Client` fields can grow but never reach `memory_report()`:\n{}\n\n\
         Add them to the report (see agent_docs/observability.md), or to EXEMPT in \
         this file with the reason they are not a per-session memory question.",
        missing.join("\n"),
    );
}

/// Per-client state a subsystem parks on the core, with the file
/// whose `memory` hook has to account for it.
///
/// The `Client` walk above cannot see these: the table stores state as
/// `Arc<dyn Any>`, so the traversal bottoms out at the erased pointer and every
/// collection behind it reads as absent. Without this list, moving a growable
/// field into a subsystem is how it would leave the report unnoticed.
const SUBSYSTEM_STATES: &[(&str, &str)] = &[
    ("VoipState", "src/voip/state.rs"),
    ("PasskeyState", "src/passkey/flow.rs"),
];

fn struct_named(text: &str, name: &str) -> Option<syn::ItemStruct> {
    fn find(items: Vec<syn::Item>, name: &str) -> Option<syn::ItemStruct> {
        for item in items {
            match item {
                syn::Item::Struct(item) if item.ident == name => return Some(item),
                syn::Item::Mod(item) => {
                    if let Some((_, inner)) = item.content
                        && let Some(found) = find(inner, name)
                    {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
    find(syn::parse_file(text).ok()?.items, name)
}

#[test]
fn every_growable_subsystem_field_reaches_the_memory_report() {
    let (defs, aliases) = crate_type_fields();
    let mut missing = Vec::new();

    for (state, file) in SUBSYSTEM_STATES {
        let text = read(file);
        let Some(item) = struct_named(&text, state) else {
            // The subsystem is not compiled into this configuration.
            continue;
        };
        let hook = fn_body(&text, "fn memory(").unwrap_or_default();
        for field in &item.fields {
            let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
                continue;
            };
            let mut idents = Vec::new();
            type_idents(&field.ty, &mut idents);
            let idents = resolve_aliases(&idents, &aliases);
            let Some(via) = growable_path(&idents, &defs) else {
                continue;
            };
            if !mentions(&hook, &name) {
                missing.push(format!("  {state}::{name} in {file} (via {via})"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these subsystem fields can grow but their `memory` hook never walks them:\n{}\n\n\
         The `Client` scan stops one level in, at `Subsystems` itself, so this \
         list is the only thing that would notice.",
        missing.join("\n"),
    );
}

#[test]
fn a_mention_is_a_whole_identifier() {
    assert!(mentions(
        "let recent_messages = self.recent_messages",
        "recent_messages"
    ));
    assert!(mentions(
        "self\n    .pending_outgoing_calls\n",
        "pending_outgoing_calls"
    ));
    // The case the boundary check exists for.
    assert!(!mentions(
        "group_cache: self.group_cache.entry_count(),",
        "cache"
    ));
    assert!(!mentions("self.group_devices_memo", "devices_memo"));
}

/// The capability the scan gained: a collection wrapped in a crate-local type,
/// or spelled through an alias, still puts its field on the candidate list.
#[test]
fn a_newtype_or_alias_does_not_hide_its_collection() {
    let (defs, aliases) = crate_type_fields();
    let path = |ident: &str| growable_path(&resolve_aliases(&[ident.to_string()], &aliases), &defs);

    // A field that names an alias directly — `group_cache: OnceLock<Arc<
    // GroupCache>>` — resolves through it, so dropping that cache from the
    // report fails the guard rather than passing silently.
    assert_eq!(
        path("GroupCache").as_deref(),
        Some("TypedCache"),
        "`type GroupCache = TypedCache<..>` must resolve on a Client field type"
    );
    assert_eq!(
        path("PendingDeviceSync").as_deref(),
        Some("PendingDeviceSync::HashSet"),
        "the offline unknown-device queue is a HashSet behind a newtype"
    );
    assert_eq!(
        path("InboundCommitBatcher").as_deref(),
        Some("InboundCommitBatcher::Vec"),
        "the offline-drain commit batch is a Vec behind a newtype"
    );
    assert_eq!(
        path("MsgSecretWriteBuffer").as_deref(),
        Some("MsgSecretWriteBuffer::HashMap"),
        "`type Pending = HashMap<..>` must not read as an opaque type"
    );

    // A type that holds no collection stays off the list, so the resolution is
    // selective rather than flagging every crate-local field type.
    assert_eq!(path("SentFrameTap"), None);
    assert_eq!(path("NotAClientFieldTypeAnywhere"), None);
}

/// An alias chain longer than any fixed number of rounds still bottoms out.
///
/// Synthetic rather than drawn from the tree: the real chains are one link, and
/// the failure this guards against — a longer chain silently reading as "no
/// collection here" — would arrive with the code that introduced it.
#[test]
fn alias_chains_resolve_to_a_fixed_point() {
    let mut aliases: HashMap<String, Vec<String>> = [
        ("A", vec!["B"]),
        ("B", vec!["C"]),
        ("C", vec!["D"]),
        ("D", vec!["E"]),
        ("E", vec!["Arc", "Vec", "u8"]),
        // Self-shadowing: `type Cache = Cache<..>` re-exports a foreign name and
        // must not expand forever.
        ("Cache", vec!["Cache"]),
    ]
    .into_iter()
    .map(|(name, target)| {
        (
            name.to_string(),
            target.into_iter().map(str::to_string).collect(),
        )
    })
    .collect();

    flatten_alias_chains(&mut aliases);

    assert_eq!(aliases["A"], ["Arc", "Vec", "u8"]);
    assert_eq!(aliases["Cache"], ["Cache"]);
    assert_eq!(
        resolve_aliases(&["A".to_string()], &aliases),
        ["Arc", "Vec", "u8"],
        "a field naming the head of the chain must reach the Vec at its end"
    );
}

/// An exemption must name a field that still exists, so the list cannot rot
/// into permission for a field nobody checks any more.
#[test]
fn every_exemption_still_names_a_client_field() {
    let client = client_struct(&read("src/client.rs"));
    let fields: Vec<String> = client
        .fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(ToString::to_string))
        .collect();

    for (name, reason) in EXEMPT {
        assert!(
            fields.iter().any(|f| f == name),
            "EXEMPT names `{name}` ({reason}), which is no longer a `Client` field"
        );
    }
}
