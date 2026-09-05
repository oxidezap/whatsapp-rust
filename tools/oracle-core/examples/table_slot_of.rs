//! Which table slot holds a given function, and what else sits beside it.
//!
//! `oracle callers` answers "who calls this" and gives up when the answer is
//! "nothing directly — it is reached through the table". This closes that gap
//! from the other side: read the element section, and report every slot the
//! function is placed in.
//!
//! That is the number the rest of the investigation needs. With it, `oracle
//! konst <slot>` finds the code that loads the index, and a host can call the
//! slot itself rather than waiting for something to dispatch through it.
//!
//! ```sh
//! cargo run --release --example table_slot_of -- S_ivh1PriOA 855
//! cargo run --release --example table_slot_of -- S_ivh1PriOA 855 --window 4
//! ```
use anyhow::{Context, Result, bail};
use oracle_core::Catalog;
use std::collections::BTreeMap;
use wasmparser::{ElementItems, ElementKind, Operator, Parser, Payload};

/// Every active table slot, as `slot -> function index`.
///
/// Only active segments with a constant offset place a function at a knowable
/// slot; a passive or expression-offset segment is skipped rather than guessed
/// at, and the count of those is reported so a short table is visible.
fn table(bytes: &[u8]) -> Result<(BTreeMap<u32, u32>, usize)> {
    let mut slots = BTreeMap::new();
    let mut skipped = 0;

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::ElementSection(reader) = payload.context("parsing the module")? else {
            continue;
        };
        for element in reader {
            let element = element.context("element segment")?;
            let ElementKind::Active { offset_expr, .. } = element.kind else {
                skipped += 1;
                continue;
            };
            let Some(base) = const_offset(&offset_expr) else {
                skipped += 1;
                continue;
            };
            let ElementItems::Functions(items) = element.items else {
                skipped += 1;
                continue;
            };
            for (i, func) in items.into_iter().enumerate() {
                slots.insert(base + i as u32, func.context("element item")?);
            }
        }
    }

    Ok((slots, skipped))
}

/// The `i32.const` an active segment's offset expression is, if it is one.
fn const_offset(expr: &wasmparser::ConstExpr<'_>) -> Option<u32> {
    let mut reader = expr.get_operators_reader();
    match reader.read().ok()? {
        Operator::I32Const { value } => u32::try_from(value).ok(),
        _ => None,
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let target = args
        .next()
        .context("usage: table_slot_of <module> <func> [--window N]")?;
    let wanted: u32 = args
        .next()
        .context("a function index")?
        .parse()
        .context("the function index must be a number")?;
    let mut window = 0u32;
    while let Some(arg) = args.next() {
        if arg == "--window" {
            window = args.next().context("--window needs a number")?.parse()?;
        } else {
            bail!("unknown argument: {arg}");
        }
    }

    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&target)?;
    let bytes = std::fs::read(&entry.path)?;
    let (slots, skipped) = table(&bytes)?;

    println!(
        "{}: {} active table slots{}",
        entry.id,
        slots.len(),
        if skipped > 0 {
            format!(", {skipped} segment(s) skipped — not active with a constant offset")
        } else {
            String::new()
        }
    );

    let found: Vec<u32> = slots
        .iter()
        .filter(|(_, func)| **func == wanted)
        .map(|(slot, _)| *slot)
        .collect();

    if found.is_empty() {
        println!("function #{wanted} is in no table slot — nothing can reach it indirectly");
        return Ok(());
    }

    println!("function #{wanted} is at slot(s): {found:?}");
    println!("  next: oracle konst {} {}", entry.id, found[0]);

    // The neighbourhood, when asked for. A dispatcher usually places a family
    // of related callbacks together, so what sits beside a slot says something
    // about who fills it.
    if window > 0 {
        for slot in &found {
            println!("\n  around slot {slot}:");
            for probe in slot.saturating_sub(window)..=slot + window {
                if let Some(func) = slots.get(&probe) {
                    let mark = if probe == *slot { " <-" } else { "" };
                    println!("    [{probe}] -> #{func}{mark}");
                }
            }
        }
    }

    Ok(())
}
