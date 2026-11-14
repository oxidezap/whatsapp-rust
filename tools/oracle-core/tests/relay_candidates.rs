//! Executes the pinned relay-list primitive, not a live allocation or full election.
use anyhow::{Result, ensure};
use oracle_core::Runtime;
use sha2::{Digest, Sha256};
use wasmtime::Val;
mod common;

// Exact encoded-body selector: indices/slots are not evidence by themselves.
const ADD_BODY: &str = "9a967455d24c06a19e63bd9dd785fd24259cfa74abe028bfd9e9089ced5297dc";
// f4600 (`pj_log_internal`) loads its threshold from this capture-specific word.
// A negative threshold takes its early return before the logger's uninitialized TLS.
const LOGGER_BODY: &str = "5a25bc62ee999fcf7bc13524767a5129548a168e88c28702b87155b19efff86e";
const LOG_LEVEL: u32 = 1_427_744;
const LIST: u32 = 3_000_000;
const ADDRESS: u32 = 3_002_000;
const NAME: u32 = 3_002_100;

fn runtime() -> Result<Option<Runtime>> {
    let Some(bytes) = common::capture("u-Bw8hwXJGE")? else {
        eprintln!("skipping: missing pinned xpop capture; run cargo xt oracle fetch");
        return Ok(None);
    };
    let mut matched = false;
    let mut logger_matched = false;
    let mut function_index = 0;
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        match payload? {
            wasmparser::Payload::ImportSection(imports) => {
                for import in imports.into_imports() {
                    if matches!(import?.ty, wasmparser::TypeRef::Func(_)) {
                        function_index += 1;
                    }
                }
            }
            wasmparser::Payload::CodeSectionEntry(body) => {
                if function_index == 14350 || function_index == 4600 {
                    let range = body.range();
                    let start = usize::try_from(range.start)?;
                    let end = usize::try_from(range.end)?;
                    let hash = hex::encode(Sha256::digest(&bytes[start..end]));
                    if function_index == 14350 {
                        matched = hash == ADD_BODY;
                    } else {
                        logger_matched = hash == LOGGER_BODY;
                    }
                }
                function_index += 1;
            }
            _ => {}
        }
    }
    ensure!(matched, "relay insertion body selector did not match");
    ensure!(logger_matched, "logger threshold selector did not match");
    // No constructors or worker threads. Disable logging explicitly, rather than
    // inventing logger TLS or stubbing a callback; the pinned dispatcher exits early.
    // Scratch is bounded within the imported memory and initialized before each probe.
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.write_bytes_at(LOG_LEVEL, &(-1i32).to_le_bytes())?;
    runtime.write_bytes_at(LIST, &[0; 4 + 8 * 164])?;
    runtime.write_bytes_at(ADDRESS, &[0; 24])?;
    runtime.write_bytes_at(ADDRESS, &[192, 0, 2, 1])?;
    runtime.write_bytes_at(ADDRESS + 20, &[0x0d, 0x96])?;
    runtime.write_bytes_at(NAME, b"synthetic\0")?;
    ensure!(
        runtime.shared().total_calls() == 0,
        "instantiation recorded a host import"
    );
    Ok(Some(runtime))
}

fn add(runtime: &mut Runtime, relay: i32, token: i32, xpop: i32, ipv6: bool) -> Result<i32> {
    let args = [
        relay,
        token,
        1,
        0,
        i32::from(!ipv6),
        i32::from(ipv6),
        xpop,
        52,
        -1,
        -1,
        NAME as i32,
        0,
        ADDRESS as i32,
        LIST as i32,
    ]
    .map(Val::I32);
    let result = runtime.call_table(10165, &args)?;
    eprintln!(
        "relay insertion result={result:?} recorded_imports={:?}",
        runtime.shared().hot_calls()
    );
    ensure!(
        runtime.shared().total_calls() == 0,
        "primitive recorded a host import"
    );
    ensure!(runtime.stubs_called().is_empty(), "primitive used a stub");
    result[0]
        .i32()
        .ok_or_else(|| anyhow::anyhow!("non-i32 relay insertion result"))
}

#[test]
fn normal_and_xpop_are_independent_identities() -> Result<()> {
    let _guard = common::threaded_guard();
    let Some(mut runtime) = runtime()? else {
        return Ok(());
    };
    assert_eq!(add(&mut runtime, 2, 2, 1, false)?, 0);
    assert_eq!(add(&mut runtime, 2, 1, 0, false)?, 0);
    assert_eq!(runtime.read(LIST, 1)?, [2]);
    let items = runtime.read(LIST + 4, 328)?;
    assert_eq!((items[0], items[1], items[52]), (2, 2, 1));
    assert_eq!(u32::from_le_bytes(items[56..60].try_into()?), 52);
    assert_eq!((items[164], items[165], items[216]), (2, 1, 0));
    assert_eq!(u32::from_le_bytes(items[220..224].try_into()?), 0);
    // The same identity cannot silently acquire a different token.
    assert_eq!(add(&mut runtime, 2, 3, 1, false)?, 70004);
    assert_eq!(runtime.read(LIST, 1)?, [2]);
    assert_eq!(runtime.read(LIST + 5, 1)?, [2]);
    Ok(())
}

#[test]
fn same_token_does_not_collapse_normal_and_xpop_and_list_is_bounded() -> Result<()> {
    let _guard = common::threaded_guard();
    let Some(mut runtime) = runtime()? else {
        return Ok(());
    };
    for relay in 0..4 {
        assert_eq!(add(&mut runtime, relay, 1, 1, false)?, 0);
        assert_eq!(add(&mut runtime, relay, 1, 0, false)?, 0);
    }
    assert_eq!(runtime.read(LIST, 1)?, [8]);
    assert_eq!(add(&mut runtime, 4, 1, 0, false)?, 70020);
    assert_eq!(runtime.read(LIST, 1)?, [8]);
    Ok(())
}

#[test]
fn address_update_preserves_other_family_and_replaces_same_family() -> Result<()> {
    let _guard = common::threaded_guard();
    let Some(mut runtime) = runtime()? else {
        return Ok(());
    };
    assert_eq!(add(&mut runtime, 2, 2, 1, false)?, 0);
    runtime.write_bytes_at(
        ADDRESS + 4,
        &[0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    )?;
    runtime.write_bytes_at(ADDRESS + 22, &[0x0d, 0x96])?;
    assert_eq!(add(&mut runtime, 2, 2, 1, true)?, 0);
    runtime.write_bytes_at(ADDRESS, &[192, 0, 2, 2])?;
    assert_eq!(add(&mut runtime, 2, 2, 1, false)?, 0);
    let item = runtime.read(LIST + 4, 164)?;
    assert_eq!(&item[4..8], &[192, 0, 2, 2]);
    assert_eq!(&item[8..12], &[0x20, 0x01, 0x0d, 0xb8]);
    assert_eq!(runtime.read(LIST, 1)?, [1]);
    Ok(())
}
